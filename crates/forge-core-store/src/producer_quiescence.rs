//! Cross-process, writer-preferring admission for durable state producers.
//!
//! Producers briefly take the gate exclusively, then retain root authority and
//! drain shared before releasing the gate. Effect producers retain authority
//! exclusively; quiescence retains gate, authority, and drain exclusively. On
//! Unix the pinned state-root directory inode is the authority. On Windows the
//! authority is a descriptor-relative direct-root regular file, retained without
//! `FILE_SHARE_DELETE`. Replacing `locks/` cannot split authority; on Unix a
//! direct authority-leaf replacement neither creates a lock domain nor invalidates
//! a lease that retains the root-directory inode.

use fs4::{FileExt, TryLockError};
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

pub const PRODUCER_GATE_LOCK: &str = "locks/producer-admission.gate.lock";
pub const PRODUCER_DRAIN_LOCK: &str = "locks/producer-admission.drain.lock";
pub const PRODUCER_ROOT_AUTHORITY_LOCK: &str = ".producer-root-authority.lock";

const EFFECT_DEADLINE: Duration = Duration::from_secs(30);
const PRODUCER_DEADLINE: Duration = Duration::from_secs(4);
const QUIESCENCE_DEADLINE: Duration = Duration::from_secs(30);
const WAIT_SLICE: Duration = Duration::from_millis(10);

#[cfg(test)]
const TEST_ROOT_AUTHORITY_GATE_ACQUIRED: &str = "FORGE_TEST_ROOT_AUTHORITY_GATE_ACQUIRED";
#[cfg(test)]
const TEST_ROOT_AUTHORITY_PROCEED: &str = "FORGE_TEST_ROOT_AUTHORITY_PROCEED";
#[cfg(test)]
const TEST_ROOT_AUTHORITY_ATTEMPT: &str = "FORGE_TEST_ROOT_AUTHORITY_ATTEMPT";
#[cfg(test)]
const TEST_ROOT_AUTHORITY_WOULD_BLOCK: &str = "FORGE_TEST_ROOT_AUTHORITY_WOULD_BLOCK";

#[cfg(test)]
fn test_root_authority_sync_paths() -> Option<(PathBuf, PathBuf, PathBuf, PathBuf)> {
    Some((
        PathBuf::from(std::env::var_os(TEST_ROOT_AUTHORITY_GATE_ACQUIRED)?),
        PathBuf::from(std::env::var_os(TEST_ROOT_AUTHORITY_PROCEED)?),
        PathBuf::from(std::env::var_os(TEST_ROOT_AUTHORITY_ATTEMPT)?),
        PathBuf::from(std::env::var_os(TEST_ROOT_AUTHORITY_WOULD_BLOCK)?),
    ))
}

#[cfg(test)]
fn synchronize_test_root_authority_acquisition() {
    let Some((gate_acquired, proceed, attempt, _)) = test_root_authority_sync_paths() else {
        return;
    };
    fs::write(&gate_acquired, b"gate-acquired").expect("publish root-authority gate marker");
    let deadline = Instant::now() + Duration::from_secs(6);
    while !proceed.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for root-authority test proceed barrier {}",
            proceed.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    fs::write(&attempt, b"root-authority-attempt").expect("publish root-authority attempt marker");
}

#[cfg(test)]
fn mark_test_root_authority_would_block() {
    let Some((_, _, _, blocked)) = test_root_authority_sync_paths() else {
        return;
    };
    fs::write(blocked, b"would-block").expect("publish root-authority WouldBlock marker");
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProducerBoundaryError {
    StateRootUnavailable { path: PathBuf, source: String },
    UnsafeStateRoot { path: PathBuf, source: String },
    CreateLockDirectory { path: PathBuf, source: String },
    OpenLock { path: PathBuf, source: String },
    UnsafeLockFile { path: PathBuf, source: String },
    Lock { path: PathBuf, source: String },
    DeadlineExceeded { path: PathBuf },
    Cancelled { path: PathBuf },
    RootIdentityChanged { path: PathBuf },
    BoundaryRootMismatch { expected: PathBuf, actual: PathBuf },
    ReentrantUpgrade { path: PathBuf },
    EffectAuthorityHeld { path: PathBuf },
    Quiescing { path: PathBuf },
    NestedExclusive { path: PathBuf },
    RegistryPoisoned,
}

impl fmt::Display for ProducerBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateRootUnavailable { path, source } => {
                write!(
                    formatter,
                    "state root {} is unavailable: {source}",
                    path.display()
                )
            }
            Self::UnsafeStateRoot { path, source } => {
                write!(
                    formatter,
                    "state root {} is unsafe: {source}",
                    path.display()
                )
            }
            Self::CreateLockDirectory { path, source } => write!(
                formatter,
                "create producer lock directory {} failed: {source}",
                path.display()
            ),
            Self::OpenLock { path, source } => {
                write!(
                    formatter,
                    "open producer lock {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsafeLockFile { path, source } => {
                write!(
                    formatter,
                    "producer lock {} is unsafe: {source}",
                    path.display()
                )
            }
            Self::Lock { path, source } => {
                write!(
                    formatter,
                    "lock producer boundary {} failed: {source}",
                    path.display()
                )
            }
            Self::DeadlineExceeded { path } => {
                write!(
                    formatter,
                    "producer boundary deadline exceeded at {}",
                    path.display()
                )
            }
            Self::Cancelled { path } => {
                write!(
                    formatter,
                    "producer boundary acquisition cancelled at {}",
                    path.display()
                )
            }
            Self::RootIdentityChanged { path } => {
                write!(
                    formatter,
                    "state root identity changed at {}",
                    path.display()
                )
            }
            Self::BoundaryRootMismatch { expected, actual } => write!(
                formatter,
                "producer boundary protects {}, not {}",
                expected.display(),
                actual.display()
            ),
            Self::ReentrantUpgrade { path } => write!(
                formatter,
                "cannot upgrade this process's shared producer admission for {}",
                path.display()
            ),
            Self::Quiescing { path } => write!(
                formatter,
                "this process already holds quiescence for {}",
                path.display()
            ),
            Self::EffectAuthorityHeld { path } => write!(
                formatter,
                "this process already holds effect authority for {}",
                path.display()
            ),
            Self::NestedExclusive { path } => write!(
                formatter,
                "this process already owns exclusive producer quiescence for {}",
                path.display()
            ),
            Self::RegistryPoisoned => formatter.write_str("producer boundary registry is poisoned"),
        }
    }
}

impl std::error::Error for ProducerBoundaryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RootIdentity {
    first: u64,
    second: u64,
}

pub(crate) struct RootPin {
    canonical: PathBuf,
    requested: PathBuf,
    identity: RootIdentity,
    locks_identity: RootIdentity,
    #[cfg(not(unix))]
    authority_identity: RootIdentity,
    gate_identity: RootIdentity,
    drain_identity: RootIdentity,
    directory: File,
    locks_directory: File,
    #[cfg(not(unix))]
    authority: File,
    gate: File,
    drain: File,
}

impl fmt::Debug for RootPin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootPin")
            .field("canonical", &self.canonical)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

struct SharedLease {
    pin: RootPin,
    registry_id: u64,
}

impl fmt::Debug for SharedLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedLease")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

impl Drop for SharedLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.pin.drain);
        release_root_authority_lock(&self.pin);
        remove_registry_entry(self.pin.identity, self.registry_id);
    }
}

struct EffectLease {
    pin: RootPin,
    owner: std::thread::ThreadId,
    registry_id: u64,
}

impl fmt::Debug for EffectLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EffectLease")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

impl Drop for EffectLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.pin.drain);
        release_root_authority_lock(&self.pin);
        remove_registry_entry(self.pin.identity, self.registry_id);
    }
}

struct ExclusiveLease {
    pin: RootPin,
    registry_id: u64,
}

impl fmt::Debug for ExclusiveLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExclusiveLease")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

impl Drop for ExclusiveLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.pin.drain);
        release_root_authority_lock(&self.pin);
        let _ = FileExt::unlock(&self.pin.gate);
        remove_registry_entry(self.pin.identity, self.registry_id);
    }
}

#[derive(Clone, Debug)]
enum ProducerLease {
    Shared(Arc<SharedLease>),
    Effect(Arc<EffectLease>),
}

#[derive(Clone, Debug)]
pub struct ProducerAdmissionGuard {
    lease: ProducerLease,
}

#[doc(hidden)]
#[derive(Debug)]
pub struct EffectProducerGuard {
    lease: Arc<EffectLease>,
}

#[derive(Debug)]
pub struct HostQuiescenceGuard {
    lease: Arc<ExclusiveLease>,
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct BoundaryLease {
    lease: BoundaryLeaseInner,
}

#[derive(Clone, Debug)]
enum BoundaryLeaseInner {
    Effect(Arc<EffectLease>),
    SharedUnderEffect(Arc<EffectLease>),
    Shared(Arc<SharedLease>),
    Exclusive(Arc<ExclusiveLease>),
}

mod sealed {
    use super::{BoundaryLease, Path, ProducerBoundaryError};

    pub trait Sealed {
        fn validate_root(&self, state_root: &Path) -> Result<(), ProducerBoundaryError>;
        fn retained_lease(&self) -> BoundaryLease;
    }
}

/// Sealed proof that the producer boundary for one exact state root is held.
///
/// Only opaque guards returned by this module implement this trait.
pub trait ProducerBoundary: sealed::Sealed {}

impl sealed::Sealed for ProducerAdmissionGuard {
    fn validate_root(&self, state_root: &Path) -> Result<(), ProducerBoundaryError> {
        match &self.lease {
            ProducerLease::Shared(lease) => validate_boundary_root_pin(&lease.pin, state_root),
            ProducerLease::Effect(lease) => validate_boundary_root_pin(&lease.pin, state_root),
        }
    }

    fn retained_lease(&self) -> BoundaryLease {
        let lease = match &self.lease {
            ProducerLease::Shared(lease) => BoundaryLeaseInner::Shared(Arc::clone(lease)),
            ProducerLease::Effect(lease) => {
                BoundaryLeaseInner::SharedUnderEffect(Arc::clone(lease))
            }
        };
        BoundaryLease { lease }
    }
}

impl sealed::Sealed for EffectProducerGuard {
    fn validate_root(&self, state_root: &Path) -> Result<(), ProducerBoundaryError> {
        validate_boundary_root_pin(&self.lease.pin, state_root)
    }

    fn retained_lease(&self) -> BoundaryLease {
        BoundaryLease {
            lease: BoundaryLeaseInner::Effect(Arc::clone(&self.lease)),
        }
    }
}
impl ProducerBoundary for EffectProducerGuard {}
impl ProducerBoundary for ProducerAdmissionGuard {}

impl sealed::Sealed for HostQuiescenceGuard {
    fn validate_root(&self, state_root: &Path) -> Result<(), ProducerBoundaryError> {
        validate_boundary_root_pin(&self.lease.pin, state_root)
    }

    fn retained_lease(&self) -> BoundaryLease {
        BoundaryLease {
            lease: BoundaryLeaseInner::Exclusive(Arc::clone(&self.lease)),
        }
    }
}
impl ProducerBoundary for HostQuiescenceGuard {}

impl BoundaryLease {
    pub(crate) fn from_boundary(
        boundary: &impl ProducerBoundary,
        state_root: &Path,
    ) -> Result<Self, ProducerBoundaryError> {
        boundary.validate_root(state_root)?;
        Ok(boundary.retained_lease())
    }
    pub(crate) fn validate_root(&self, state_root: &Path) -> Result<(), ProducerBoundaryError> {
        validate_boundary_root_pin(self.pin(), state_root)
    }

    pub(crate) fn require_effect_authority(&self) -> Result<(), ProducerBoundaryError> {
        match &self.lease {
            BoundaryLeaseInner::Effect(_) | BoundaryLeaseInner::Exclusive(_) => Ok(()),
            BoundaryLeaseInner::SharedUnderEffect(lease) => {
                Err(ProducerBoundaryError::ReentrantUpgrade {
                    path: lease.pin.canonical.clone(),
                })
            }
            BoundaryLeaseInner::Shared(lease) => Err(ProducerBoundaryError::ReentrantUpgrade {
                path: lease.pin.canonical.clone(),
            }),
        }
    }

    pub(crate) fn retained_root(
        &self,
    ) -> Result<crate::retained_dir::RetainedDirectory, ProducerBoundaryError> {
        let pin = self.pin();
        let handle = pin.directory.try_clone().map_err(|source| {
            ProducerBoundaryError::StateRootUnavailable {
                path: pin.canonical.clone(),
                source: source.to_string(),
            }
        })?;
        Ok(crate::retained_dir::RetainedDirectory::from_handle(
            handle,
            pin.canonical.clone(),
        ))
    }

    fn pin(&self) -> &RootPin {
        match &self.lease {
            BoundaryLeaseInner::Effect(lease) | BoundaryLeaseInner::SharedUnderEffect(lease) => {
                &lease.pin
            }
            BoundaryLeaseInner::Shared(lease) => &lease.pin,
            BoundaryLeaseInner::Exclusive(lease) => &lease.pin,
        }
    }
}

enum RegistryEntry {
    Admitting {
        id: u64,
        path: PathBuf,
    },
    Shared {
        id: u64,
        path: PathBuf,
        lease: Weak<SharedLease>,
    },
    Effect {
        id: u64,
        path: PathBuf,
        owner: std::thread::ThreadId,
        lease: Weak<EffectLease>,
    },
    Exclusive {
        id: u64,
        path: PathBuf,
        lease: Weak<ExclusiveLease>,
    },
}

#[derive(Default)]
struct RegistryState {
    next_id: u64,
    roots: HashMap<RootIdentity, RegistryEntry>,
}

#[derive(Default)]
struct Registry {
    state: Mutex<RegistryState>,
    changed: Condvar,
}

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Registry::default)
}

fn next_registry_id(state: &mut RegistryState) -> u64 {
    state.next_id = state.next_id.wrapping_add(1).max(1);
    state.next_id
}

fn remove_registry_entry(identity: RootIdentity, id: u64) {
    let registry = registry();
    if let Ok(mut state) = registry.state.lock() {
        let matches = match state.roots.get(&identity) {
            Some(
                RegistryEntry::Admitting { id: current, .. }
                | RegistryEntry::Effect { id: current, .. }
                | RegistryEntry::Shared { id: current, .. }
                | RegistryEntry::Exclusive { id: current, .. },
            ) => *current == id,
            None => false,
        };
        if matches {
            state.roots.remove(&identity);
            registry.changed.notify_all();
        }
    }
}

fn has_active_path_conflict(state: &RegistryState, pin: &RootPin) -> bool {
    state.roots.iter().any(|(identity, entry)| {
        if *identity == pin.identity {
            return false;
        }
        match entry {
            RegistryEntry::Admitting { path, .. } => path == &pin.canonical,
            RegistryEntry::Shared { path, lease, .. } => {
                path == &pin.canonical && lease.strong_count() > 0
            }
            RegistryEntry::Effect { path, lease, .. } => {
                path == &pin.canonical && lease.strong_count() > 0
            }
            RegistryEntry::Exclusive { path, lease, .. } => {
                path == &pin.canonical && lease.strong_count() > 0
            }
        }
    })
}

/// Admit a durable producer for an existing, non-symlink state root.
///
/// Acquisition has a fixed four-second deadline. Nested and sibling calls in
/// this process share one OS lease for the same pinned root identity.
pub fn admit_producer(
    state_root: impl AsRef<Path>,
) -> Result<ProducerAdmissionGuard, ProducerBoundaryError> {
    static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
    admit_producer_with_cancellation(state_root, &NEVER_CANCELLED)
}

/// Admit a producer with cancellation in addition to the trusted deadline.
pub fn admit_producer_with_cancellation(
    state_root: impl AsRef<Path>,
    cancellation: &AtomicBool,
) -> Result<ProducerAdmissionGuard, ProducerBoundaryError> {
    let pin = pin_state_root(state_root.as_ref())?;
    let deadline = Instant::now() + PRODUCER_DEADLINE;
    admit_pinned(pin, cancellation, deadline)
}

#[doc(hidden)]
pub fn admit_effect_producer(
    state_root: &Path,
    try_only: bool,
) -> Result<EffectProducerGuard, ProducerBoundaryError> {
    static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
    let pin = pin_state_root(state_root)?;
    let deadline = Instant::now()
        + if try_only {
            PRODUCER_DEADLINE
        } else {
            EFFECT_DEADLINE
        };
    admit_effect_pinned(pin, &NEVER_CANCELLED, deadline, try_only)
}

/// Close producer admission and drain all already-admitted producers.
///
/// This is hidden from generated API documentation because only trusted
/// backup orchestration should acquire host quiescence.
#[doc(hidden)]
pub fn quiesce_host_producers(
    state_root: impl AsRef<Path>,
    cancellation: &AtomicBool,
) -> Result<HostQuiescenceGuard, ProducerBoundaryError> {
    let pin = pin_state_root(state_root.as_ref())?;
    let deadline = Instant::now() + QUIESCENCE_DEADLINE;
    quiesce_pinned(pin, cancellation, deadline)
}

fn admit_pinned(
    pin: RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<ProducerAdmissionGuard, ProducerBoundaryError> {
    let registry = registry();
    let id;
    loop {
        check_wait(cancellation, deadline, &pin.canonical)?;
        let mut state = registry
            .state
            .lock()
            .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
        if has_active_path_conflict(&state, &pin) {
            return Err(ProducerBoundaryError::RootIdentityChanged {
                path: pin.canonical.clone(),
            });
        }
        match state.roots.get(&pin.identity) {
            Some(RegistryEntry::Shared { lease, .. }) if lease.strong_count() > 0 => {
                // The registry mutex serializes process-local probes. The OS
                // probe itself is nonblocking, so a closed external gate is
                // rejected without retaining another shared Arc or deadlocking
                // the writer that is waiting for this process to drain.
                try_lock_gate_for_reuse(&pin, cancellation, deadline)?;
                let reused = lease.upgrade();
                if let Some(lease) = reused {
                    let validation = validate_pin(&lease.pin);
                    let _ = FileExt::unlock(&pin.gate);
                    // Keep probe ownership and registry serialization atomic: a
                    // sibling must never observe this process's probe as a writer.
                    drop(state);
                    validation?;
                    return Ok(ProducerAdmissionGuard {
                        lease: ProducerLease::Shared(lease),
                    });
                }
                let _ = FileExt::unlock(&pin.gate);
                state.roots.remove(&pin.identity);
                continue;
            }
            Some(RegistryEntry::Shared { .. }) => {
                state.roots.remove(&pin.identity);
            }
            Some(RegistryEntry::Effect { lease, .. }) => {
                // Reusing an effect lease is still a new producer admission.
                // Probe the external gate while the registry mutex prevents a
                // process-local sibling from extending the lease concurrently.
                try_lock_gate_for_reuse(&pin, cancellation, deadline)?;
                let reused = lease.upgrade();
                if let Some(lease) = reused {
                    let validation = validate_pin(&lease.pin);
                    let _ = FileExt::unlock(&pin.gate);
                    drop(state);
                    validation?;
                    return Ok(ProducerAdmissionGuard {
                        lease: ProducerLease::Effect(lease),
                    });
                }
                let _ = FileExt::unlock(&pin.gate);
                state.roots.remove(&pin.identity);
            }
            Some(RegistryEntry::Exclusive { lease, .. }) => {
                if lease.strong_count() > 0 {
                    return Err(ProducerBoundaryError::Quiescing {
                        path: pin.canonical.clone(),
                    });
                }
                state.roots.remove(&pin.identity);
            }
            Some(RegistryEntry::Admitting { .. }) => {
                let (guard, _) = registry
                    .changed
                    .wait_timeout(state, WAIT_SLICE)
                    .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
                drop(guard);
                continue;
            }
            None => {}
        }
        id = next_registry_id(&mut state);
        state.roots.insert(
            pin.identity,
            RegistryEntry::Admitting {
                id,
                path: pin.canonical.clone(),
            },
        );
        break;
    }

    let result = acquire_shared_lease(pin, cancellation, deadline, id);
    let mut state = registry
        .state
        .lock()
        .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
    match result {
        Ok(lease) => {
            state.roots.insert(
                lease.pin.identity,
                RegistryEntry::Shared {
                    id,
                    path: lease.pin.canonical.clone(),
                    lease: Arc::downgrade(&lease),
                },
            );
            registry.changed.notify_all();
            Ok(ProducerAdmissionGuard {
                lease: ProducerLease::Shared(lease),
            })
        }
        Err(error) => {
            state.roots.retain(|_, entry| !entry_has_id(entry, id));
            registry.changed.notify_all();
            Err(error)
        }
    }
}

fn admit_effect_pinned(
    pin: RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
    try_only: bool,
) -> Result<EffectProducerGuard, ProducerBoundaryError> {
    let registry = registry();
    let id;
    loop {
        check_wait(cancellation, deadline, &pin.canonical)?;
        let mut state = registry
            .state
            .lock()
            .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
        if has_active_path_conflict(&state, &pin) {
            return Err(ProducerBoundaryError::RootIdentityChanged {
                path: pin.canonical.clone(),
            });
        }
        match state.roots.get(&pin.identity) {
            Some(RegistryEntry::Shared { lease, .. }) if lease.strong_count() > 0 => {
                return Err(ProducerBoundaryError::ReentrantUpgrade {
                    path: pin.canonical.clone(),
                });
            }
            Some(RegistryEntry::Effect { owner, lease, .. }) if lease.strong_count() > 0 => {
                if *owner == std::thread::current().id() {
                    // Probe before retaining the reused Arc, exactly as shared
                    // reuse does, so a closed gate cannot extend this lease.
                    try_lock_gate_for_reuse(&pin, cancellation, deadline)?;
                    let reused = lease.upgrade();
                    if let Some(lease) = reused {
                        let validation = validate_pin(&lease.pin);
                        let _ = FileExt::unlock(&pin.gate);
                        drop(state);
                        validation?;
                        return Ok(EffectProducerGuard { lease });
                    }
                    let _ = FileExt::unlock(&pin.gate);
                    state.roots.remove(&pin.identity);
                    continue;
                }
                if try_only {
                    return Err(ProducerBoundaryError::EffectAuthorityHeld {
                        path: pin.canonical.clone(),
                    });
                }
                let (guard, _) = registry
                    .changed
                    .wait_timeout(state, WAIT_SLICE)
                    .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
                drop(guard);
                continue;
            }
            Some(RegistryEntry::Exclusive { lease, .. }) if lease.strong_count() > 0 => {
                return Err(ProducerBoundaryError::Quiescing {
                    path: pin.canonical.clone(),
                });
            }
            Some(RegistryEntry::Admitting { .. }) => {
                let (guard, _) = registry
                    .changed
                    .wait_timeout(state, WAIT_SLICE)
                    .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
                drop(guard);
                continue;
            }
            Some(_) => {
                state.roots.remove(&pin.identity);
            }
            None => {}
        }
        id = next_registry_id(&mut state);
        state.roots.insert(
            pin.identity,
            RegistryEntry::Admitting {
                id,
                path: pin.canonical.clone(),
            },
        );
        break;
    }

    let result = acquire_effect_lease(pin, cancellation, deadline, id, try_only);
    let mut state = registry
        .state
        .lock()
        .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
    match result {
        Ok(lease) => {
            state.roots.insert(
                lease.pin.identity,
                RegistryEntry::Effect {
                    id,
                    path: lease.pin.canonical.clone(),
                    owner: lease.owner,
                    lease: Arc::downgrade(&lease),
                },
            );
            registry.changed.notify_all();
            Ok(EffectProducerGuard { lease })
        }
        Err(error) => {
            state.roots.retain(|_, entry| !entry_has_id(entry, id));
            registry.changed.notify_all();
            Err(error)
        }
    }
}

fn entry_has_id(entry: &RegistryEntry, id: u64) -> bool {
    match entry {
        RegistryEntry::Admitting { id: current, .. }
        | RegistryEntry::Effect { id: current, .. }
        | RegistryEntry::Shared { id: current, .. }
        | RegistryEntry::Exclusive { id: current, .. } => *current == id,
    }
}

fn quiesce_pinned(
    pin: RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<HostQuiescenceGuard, ProducerBoundaryError> {
    let registry = registry();
    let id;
    loop {
        check_wait(cancellation, deadline, &pin.canonical)?;
        let mut state = registry
            .state
            .lock()
            .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
        if has_active_path_conflict(&state, &pin) {
            return Err(ProducerBoundaryError::RootIdentityChanged {
                path: pin.canonical.clone(),
            });
        }
        match state.roots.get(&pin.identity) {
            Some(RegistryEntry::Shared { lease, .. }) if lease.strong_count() > 0 => {
                return Err(ProducerBoundaryError::ReentrantUpgrade {
                    path: pin.canonical.clone(),
                });
            }
            Some(RegistryEntry::Effect { lease, .. }) if lease.strong_count() > 0 => {
                return Err(ProducerBoundaryError::ReentrantUpgrade {
                    path: pin.canonical.clone(),
                });
            }
            Some(RegistryEntry::Exclusive { lease, .. }) if lease.strong_count() > 0 => {
                return Err(ProducerBoundaryError::NestedExclusive {
                    path: pin.canonical.clone(),
                });
            }
            Some(RegistryEntry::Admitting { .. }) => {
                let (guard, _) = registry
                    .changed
                    .wait_timeout(state, WAIT_SLICE)
                    .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
                drop(guard);
                continue;
            }
            Some(_) => {
                state.roots.remove(&pin.identity);
            }
            None => {}
        }
        id = next_registry_id(&mut state);
        state.roots.insert(
            pin.identity,
            RegistryEntry::Admitting {
                id,
                path: pin.canonical.clone(),
            },
        );
        break;
    }

    let result = acquire_exclusive_lease(pin, cancellation, deadline, id);
    let mut state = registry
        .state
        .lock()
        .map_err(|_| ProducerBoundaryError::RegistryPoisoned)?;
    match result {
        Ok(lease) => {
            state.roots.insert(
                lease.pin.identity,
                RegistryEntry::Exclusive {
                    id,
                    path: lease.pin.canonical.clone(),
                    lease: Arc::downgrade(&lease),
                },
            );
            registry.changed.notify_all();
            Ok(HostQuiescenceGuard { lease })
        }
        Err(error) => {
            state.roots.retain(|_, entry| !entry_has_id(entry, id));
            registry.changed.notify_all();
            Err(error)
        }
    }
}
fn try_lock_gate_for_reuse(
    pin: &RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<(), ProducerBoundaryError> {
    let path = pin.canonical.join(PRODUCER_GATE_LOCK);
    check_wait(cancellation, deadline, &path)?;
    validate_pin(pin)?;
    match FileExt::try_lock(&pin.gate) {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(ProducerBoundaryError::Quiescing {
            path: pin.canonical.clone(),
        }),
        Err(TryLockError::Error(source)) => Err(ProducerBoundaryError::Lock {
            path,
            source: source.to_string(),
        }),
    }
}

fn acquire_shared_lease(
    pin: RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
    registry_id: u64,
) -> Result<Arc<SharedLease>, ProducerBoundaryError> {
    let gate_path = pin.canonical.join(PRODUCER_GATE_LOCK);
    let drain_path = pin.canonical.join(PRODUCER_DRAIN_LOCK);
    acquire_file_lock(
        &pin.gate,
        &gate_path,
        false,
        cancellation,
        deadline,
        &pin,
        LockPurpose::GateOrDrain,
    )?;
    if let Err(error) = acquire_root_authority_lock(&pin, true, cancellation, deadline) {
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    if let Err(error) = acquire_file_lock(
        &pin.drain,
        &drain_path,
        true,
        cancellation,
        deadline,
        &pin,
        LockPurpose::GateOrDrain,
    ) {
        release_root_authority_lock(&pin);
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    if let Err(error) = validate_pin(&pin) {
        let _ = FileExt::unlock(&pin.drain);
        release_root_authority_lock(&pin);
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    let _ = FileExt::unlock(&pin.gate);
    Ok(Arc::new(SharedLease { pin, registry_id }))
}

fn acquire_effect_lease(
    pin: RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
    registry_id: u64,
    try_only: bool,
) -> Result<Arc<EffectLease>, ProducerBoundaryError> {
    let gate_path = pin.canonical.join(PRODUCER_GATE_LOCK);
    let drain_path = pin.canonical.join(PRODUCER_DRAIN_LOCK);
    acquire_file_lock(
        &pin.gate,
        &gate_path,
        false,
        cancellation,
        deadline,
        &pin,
        LockPurpose::GateOrDrain,
    )?;
    #[cfg(test)]
    synchronize_test_root_authority_acquisition();
    let authority_result = if try_only {
        try_acquire_effect_root_authority_lock(&pin)
    } else {
        acquire_root_authority_lock(&pin, false, cancellation, deadline)
    };
    if let Err(error) = authority_result {
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    if let Err(error) = acquire_file_lock(
        &pin.drain,
        &drain_path,
        true,
        cancellation,
        deadline,
        &pin,
        LockPurpose::GateOrDrain,
    ) {
        release_root_authority_lock(&pin);
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    if let Err(error) = validate_pin(&pin) {
        let _ = FileExt::unlock(&pin.drain);
        release_root_authority_lock(&pin);
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    let _ = FileExt::unlock(&pin.gate);
    Ok(Arc::new(EffectLease {
        pin,
        registry_id,
        owner: std::thread::current().id(),
    }))
}

fn acquire_exclusive_lease(
    pin: RootPin,
    cancellation: &AtomicBool,
    deadline: Instant,
    registry_id: u64,
) -> Result<Arc<ExclusiveLease>, ProducerBoundaryError> {
    let gate_path = pin.canonical.join(PRODUCER_GATE_LOCK);
    let drain_path = pin.canonical.join(PRODUCER_DRAIN_LOCK);
    acquire_file_lock(
        &pin.gate,
        &gate_path,
        false,
        cancellation,
        deadline,
        &pin,
        LockPurpose::GateOrDrain,
    )?;
    #[cfg(test)]
    synchronize_test_root_authority_acquisition();
    if let Err(error) = acquire_root_authority_lock(&pin, false, cancellation, deadline) {
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    if let Err(error) = acquire_file_lock(
        &pin.drain,
        &drain_path,
        false,
        cancellation,
        deadline,
        &pin,
        LockPurpose::GateOrDrain,
    ) {
        release_root_authority_lock(&pin);
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    if let Err(error) = validate_pin(&pin) {
        let _ = FileExt::unlock(&pin.drain);
        release_root_authority_lock(&pin);
        let _ = FileExt::unlock(&pin.gate);
        return Err(error);
    }
    Ok(Arc::new(ExclusiveLease { pin, registry_id }))
}

fn acquire_root_authority_lock(
    pin: &RootPin,
    shared: bool,
    cancellation: &AtomicBool,
    deadline: Instant,
) -> Result<(), ProducerBoundaryError> {
    #[cfg(unix)]
    {
        acquire_file_lock(
            &pin.directory,
            &pin.canonical,
            shared,
            cancellation,
            deadline,
            pin,
            LockPurpose::RootAuthority,
        )
    }
    #[cfg(not(unix))]
    {
        let path = pin.canonical.join(PRODUCER_ROOT_AUTHORITY_LOCK);
        acquire_file_lock(
            &pin.authority,
            &path,
            shared,
            cancellation,
            deadline,
            pin,
            LockPurpose::RootAuthority,
        )
    }
}

fn try_acquire_effect_root_authority_lock(pin: &RootPin) -> Result<(), ProducerBoundaryError> {
    #[cfg(unix)]
    {
        let path = &pin.canonical;
        validate_pin(pin)?;
        match FileExt::try_lock(&pin.directory) {
            Ok(()) => Ok(()),
            Err(TryLockError::WouldBlock) => {
                #[cfg(test)]
                mark_test_root_authority_would_block();
                Err(ProducerBoundaryError::EffectAuthorityHeld { path: path.clone() })
            }
            Err(TryLockError::Error(source)) => Err(ProducerBoundaryError::Lock {
                path: path.clone(),
                source: source.to_string(),
            }),
        }
    }
    #[cfg(not(unix))]
    {
        let path = pin.canonical.join(PRODUCER_ROOT_AUTHORITY_LOCK);
        validate_pin(pin)?;
        match FileExt::try_lock(&pin.authority) {
            Ok(()) => Ok(()),
            Err(TryLockError::WouldBlock) => {
                #[cfg(test)]
                mark_test_root_authority_would_block();
                Err(ProducerBoundaryError::EffectAuthorityHeld { path })
            }
            Err(TryLockError::Error(source)) => Err(ProducerBoundaryError::Lock {
                path,
                source: source.to_string(),
            }),
        }
    }
}

fn release_root_authority_lock(pin: &RootPin) {
    #[cfg(unix)]
    let _ = FileExt::unlock(&pin.directory);
    #[cfg(not(unix))]
    let _ = FileExt::unlock(&pin.authority);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LockPurpose {
    GateOrDrain,
    RootAuthority,
}

fn acquire_file_lock(
    file: &File,
    path: &Path,
    shared: bool,
    cancellation: &AtomicBool,
    deadline: Instant,
    pin: &RootPin,
    purpose: LockPurpose,
) -> Result<(), ProducerBoundaryError> {
    let _ = purpose;
    let mut attempt = 0_u32;
    loop {
        check_wait(cancellation, deadline, path)?;
        validate_pin(pin)?;
        let result = if shared {
            FileExt::try_lock_shared(file)
        } else {
            FileExt::try_lock(file)
        };
        match result {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) => {
                #[cfg(test)]
                if purpose == LockPurpose::RootAuthority {
                    mark_test_root_authority_would_block();
                }
                let backoff = Duration::from_millis(2_u64 << attempt.min(5));
                std::thread::sleep(backoff.min(WAIT_SLICE));
                attempt = attempt.saturating_add(1);
            }
            Err(TryLockError::Error(source)) => {
                return Err(ProducerBoundaryError::Lock {
                    path: path.to_path_buf(),
                    source: source.to_string(),
                });
            }
        }
    }
}

fn check_wait(
    cancellation: &AtomicBool,
    deadline: Instant,
    path: &Path,
) -> Result<(), ProducerBoundaryError> {
    if cancellation.load(Ordering::Acquire) {
        return Err(ProducerBoundaryError::Cancelled {
            path: path.to_path_buf(),
        });
    }
    if Instant::now() >= deadline {
        return Err(ProducerBoundaryError::DeadlineExceeded {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn pin_state_root(requested: &Path) -> Result<RootPin, ProducerBoundaryError> {
    let lexical_metadata = fs::symlink_metadata(requested).map_err(|source| {
        ProducerBoundaryError::StateRootUnavailable {
            path: requested.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    if metadata_is_link_or_reparse(&lexical_metadata) || !lexical_metadata.is_dir() {
        return Err(ProducerBoundaryError::UnsafeStateRoot {
            path: requested.to_path_buf(),
            source: "must be a real, non-symlink directory".to_owned(),
        });
    }
    let canonical = fs::canonicalize(requested).map_err(|source| {
        ProducerBoundaryError::StateRootUnavailable {
            path: requested.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let directory = open_directory_nofollow(&canonical).map_err(|source| {
        ProducerBoundaryError::StateRootUnavailable {
            path: canonical.clone(),
            source: source.to_string(),
        }
    })?;
    let metadata =
        directory
            .metadata()
            .map_err(|source| ProducerBoundaryError::StateRootUnavailable {
                path: canonical.clone(),
                source: source.to_string(),
            })?;
    if !metadata.is_dir() || metadata_is_link_or_reparse(&metadata) {
        return Err(ProducerBoundaryError::UnsafeStateRoot {
            path: canonical,
            source: "opened state-root handle is not a real directory".to_owned(),
        });
    }
    let identity =
        metadata_identity(&metadata).ok_or_else(|| ProducerBoundaryError::UnsafeStateRoot {
            path: canonical.clone(),
            source: "filesystem does not expose a stable directory identity".to_owned(),
        })?;
    #[cfg(not(unix))]
    let (authority, authority_identity) = open_boundary_lock(
        &directory,
        &canonical,
        identity,
        PRODUCER_ROOT_AUTHORITY_LOCK,
    )?;
    #[cfg(unix)]
    let _authority = open_boundary_lock(
        &directory,
        &canonical,
        identity,
        PRODUCER_ROOT_AUTHORITY_LOCK,
    )?;
    let locks = canonical.join("locks");
    prepare_lock_directory(&locks)?;
    let locks_directory = open_directory_nofollow(&locks).map_err(|source| {
        ProducerBoundaryError::CreateLockDirectory {
            path: locks.clone(),
            source: source.to_string(),
        }
    })?;
    let locks_metadata = locks_directory.metadata().map_err(|source| {
        ProducerBoundaryError::CreateLockDirectory {
            path: locks.clone(),
            source: source.to_string(),
        }
    })?;
    if !locks_metadata.is_dir() || metadata_is_link_or_reparse(&locks_metadata) {
        return Err(ProducerBoundaryError::CreateLockDirectory {
            path: locks,
            source: "must be a real directory".to_owned(),
        });
    }
    let locks_identity = metadata_identity(&locks_metadata).ok_or_else(|| {
        ProducerBoundaryError::CreateLockDirectory {
            path: locks.clone(),
            source: "filesystem does not expose a stable directory identity".to_owned(),
        }
    })?;
    let (gate, gate_identity) = open_boundary_lock(
        &locks_directory,
        &locks,
        locks_identity,
        "producer-admission.gate.lock",
    )?;
    let (drain, drain_identity) = open_boundary_lock(
        &locks_directory,
        &locks,
        locks_identity,
        "producer-admission.drain.lock",
    )?;
    let pin = RootPin {
        requested: canonical.clone(),
        canonical,
        identity,
        locks_identity,
        #[cfg(not(unix))]
        authority_identity,
        gate_identity,
        drain_identity,
        directory,
        locks_directory,
        #[cfg(not(unix))]
        authority,
        gate,
        drain,
    };
    validate_pin(&pin)?;
    Ok(pin)
}

fn validate_boundary_root_pin(
    expected: &RootPin,
    state_root: &Path,
) -> Result<(), ProducerBoundaryError> {
    validate_pin(expected)?;
    let actual = pin_state_root(state_root)?;
    if actual.identity != expected.identity {
        return Err(ProducerBoundaryError::BoundaryRootMismatch {
            expected: expected.canonical.clone(),
            actual: actual.canonical,
        });
    }
    Ok(())
}

fn validate_pin(pin: &RootPin) -> Result<(), ProducerBoundaryError> {
    let lexical = fs::symlink_metadata(&pin.requested).map_err(|_| {
        ProducerBoundaryError::RootIdentityChanged {
            path: pin.requested.clone(),
        }
    })?;
    if metadata_is_link_or_reparse(&lexical) || !lexical.is_dir() {
        return Err(ProducerBoundaryError::RootIdentityChanged {
            path: pin.requested.clone(),
        });
    }
    let retained_root = pin
        .directory
        .metadata()
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    let current_root = open_directory_nofollow(&pin.requested)
        .and_then(|file| file.metadata())
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    if retained_root != Some(pin.identity) || current_root != Some(pin.identity) {
        return Err(ProducerBoundaryError::RootIdentityChanged {
            path: pin.requested.clone(),
        });
    }
    let locks = pin.canonical.join("locks");
    let locks_metadata = fs::symlink_metadata(&locks).ok();
    if locks_metadata
        .as_ref()
        .is_none_or(|metadata| metadata_is_link_or_reparse(metadata) || !metadata.is_dir())
    {
        return Err(ProducerBoundaryError::RootIdentityChanged { path: locks });
    }
    let retained_locks = pin
        .locks_directory
        .metadata()
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    let current_locks = open_directory_nofollow(&locks)
        .and_then(|file| file.metadata())
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    if retained_locks != Some(pin.locks_identity) || current_locks != Some(pin.locks_identity) {
        return Err(ProducerBoundaryError::RootIdentityChanged { path: locks });
    }
    #[cfg(not(unix))]
    validate_pinned_lock(
        &pin.authority,
        &pin.canonical.join(PRODUCER_ROOT_AUTHORITY_LOCK),
        pin.authority_identity,
    )?;
    validate_pinned_lock(
        &pin.gate,
        &pin.canonical.join(PRODUCER_GATE_LOCK),
        pin.gate_identity,
    )?;
    validate_pinned_lock(
        &pin.drain,
        &pin.canonical.join(PRODUCER_DRAIN_LOCK),
        pin.drain_identity,
    )?;
    Ok(())
}

fn prepare_lock_directory(locks: &Path) -> Result<(), ProducerBoundaryError> {
    match fs::symlink_metadata(locks) {
        Ok(metadata) => {
            if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
                return Err(ProducerBoundaryError::CreateLockDirectory {
                    path: locks.to_path_buf(),
                    source: "must be a real directory".to_owned(),
                });
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            if let Err(source) = fs::create_dir(locks) {
                if source.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(ProducerBoundaryError::CreateLockDirectory {
                        path: locks.to_path_buf(),
                        source: source.to_string(),
                    });
                }
            }
        }
        Err(source) => {
            return Err(ProducerBoundaryError::CreateLockDirectory {
                path: locks.to_path_buf(),
                source: source.to_string(),
            });
        }
    }
    let canonical =
        fs::canonicalize(locks).map_err(|source| ProducerBoundaryError::CreateLockDirectory {
            path: locks.to_path_buf(),
            source: source.to_string(),
        })?;
    if canonical != locks {
        return Err(ProducerBoundaryError::CreateLockDirectory {
            path: locks.to_path_buf(),
            source: "lock directory does not resolve to its pinned state-root path".to_owned(),
        });
    }
    Ok(())
}

fn open_boundary_lock(
    directory: &File,
    parent: &Path,
    parent_identity: RootIdentity,
    file_name: &str,
) -> Result<(File, RootIdentity), ProducerBoundaryError> {
    let path = parent.join(file_name);
    let retained_parent = directory
        .metadata()
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    let current_parent = open_directory_nofollow(parent)
        .and_then(|file| file.metadata())
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    if retained_parent != Some(parent_identity) || current_parent != Some(parent_identity) {
        return Err(ProducerBoundaryError::RootIdentityChanged {
            path: parent.to_path_buf(),
        });
    }
    let file = open_lock_nofollow_at(directory, &path, file_name).map_err(|source| {
        ProducerBoundaryError::OpenLock {
            path: path.clone(),
            source: source.to_string(),
        }
    })?;
    let opened = file
        .metadata()
        .map_err(|source| ProducerBoundaryError::OpenLock {
            path: path.clone(),
            source: source.to_string(),
        })?;
    validate_lock_metadata(&path, &opened)?;
    let identity =
        metadata_identity(&opened).ok_or_else(|| ProducerBoundaryError::UnsafeLockFile {
            path: path.clone(),
            source: "filesystem does not expose a stable lock-file identity".to_owned(),
        })?;
    validate_pinned_lock(&file, &path, identity)?;
    let current_parent = open_directory_nofollow(parent)
        .and_then(|current| current.metadata())
        .ok()
        .and_then(|metadata| metadata_identity(&metadata));
    if current_parent != Some(parent_identity) {
        return Err(ProducerBoundaryError::RootIdentityChanged {
            path: parent.to_path_buf(),
        });
    }
    Ok((file, identity))
}

fn validate_pinned_lock(
    file: &File,
    path: &Path,
    expected_identity: RootIdentity,
) -> Result<(), ProducerBoundaryError> {
    let opened = file
        .metadata()
        .map_err(|source| ProducerBoundaryError::UnsafeLockFile {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    validate_lock_metadata(path, &opened)?;
    let lexical =
        fs::symlink_metadata(path).map_err(|source| ProducerBoundaryError::UnsafeLockFile {
            path: path.to_path_buf(),
            source: source.to_string(),
        })?;
    validate_lock_metadata(path, &lexical)?;
    if metadata_identity(&opened) != Some(expected_identity)
        || metadata_identity(&lexical) != Some(expected_identity)
    {
        return Err(ProducerBoundaryError::UnsafeLockFile {
            path: path.to_path_buf(),
            source: "lock-file identity changed while its lease was retained".to_owned(),
        });
    }
    Ok(())
}

fn validate_lock_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ProducerBoundaryError> {
    if metadata_is_link_or_reparse(metadata) || !metadata.is_file() {
        return Err(ProducerBoundaryError::UnsafeLockFile {
            path: path.to_path_buf(),
            source: "must be a regular non-symlink file".to_owned(),
        });
    }
    if metadata_link_count(metadata).is_some_and(|count| count != 1) {
        return Err(ProducerBoundaryError::UnsafeLockFile {
            path: path.to_path_buf(),
            source: "must have exactly one filesystem link".to_owned(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn open_directory_nofollow(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY)
        .open(path)
}

#[cfg(windows)]
fn open_directory_nofollow(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_directory_nofollow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(unix)]
fn open_lock_nofollow_at(directory: &File, _path: &Path, file_name: &str) -> std::io::Result<File> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd as _, FromRawFd as _};
    use std::os::unix::ffi::OsStrExt as _;

    let name = CString::new(std::ffi::OsStr::new(file_name).as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "lock file name contains NUL",
        )
    })?;
    // SAFETY: `name` is NUL-terminated, the retained directory fd is live,
    // and a successful descriptor is transferred exactly once into `File`.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `openat` returned a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(windows)]
fn open_lock_nofollow_at(directory: &File, _path: &Path, file_name: &str) -> std::io::Result<File> {
    let retained = crate::retained_dir::RetainedDirectory::from_handle(
        directory.try_clone()?,
        PathBuf::from("."),
    );
    retained.open_retained_lock(Path::new(file_name))
}

#[cfg(not(any(unix, windows)))]
fn open_lock_nofollow_at(
    _directory: &File,
    path: &Path,
    _file_name: &str,
) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
fn metadata_identity(metadata: &fs::Metadata) -> Option<RootIdentity> {
    use std::os::unix::fs::MetadataExt as _;
    Some(RootIdentity {
        first: metadata.dev(),
        second: metadata.ino(),
    })
}

#[cfg(windows)]
fn metadata_identity(metadata: &fs::Metadata) -> Option<RootIdentity> {
    use std::os::windows::fs::MetadataExt as _;
    Some(RootIdentity {
        first: u64::from(metadata.volume_serial_number()?),
        second: metadata.file_index()?,
    })
}

#[cfg(not(any(unix, windows)))]
fn metadata_identity(_metadata: &fs::Metadata) -> Option<RootIdentity> {
    None
}

#[cfg(unix)]
fn metadata_link_count(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt as _;
    Some(metadata.nlink())
}

#[cfg(windows)]
fn metadata_link_count(metadata: &fs::Metadata) -> Option<u64> {
    use std::os::windows::fs::MetadataExt as _;
    metadata.number_of_links().map(u64::from)
}

#[cfg(not(any(unix, windows)))]
fn metadata_link_count(_metadata: &fs::Metadata) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command};
    use std::sync::atomic::AtomicU64;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "forge-producer-boundary-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create test state root");
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn wait_for(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(6);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {}",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn spawn_helper(role: &str, root: &Path, ready: &Path, release: &Path) -> Child {
        Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("producer_quiescence::tests::subprocess_entrypoint")
            .arg("--nocapture")
            .env("FORGE_BOUNDARY_HELPER_ROLE", role)
            .env("FORGE_BOUNDARY_HELPER_ROOT", root)
            .env("FORGE_BOUNDARY_HELPER_READY", ready)
            .env("FORGE_BOUNDARY_HELPER_RELEASE", release)
            .spawn()
            .expect("spawn boundary helper")
    }

    struct RootAuthoritySync {
        gate_acquired: PathBuf,
        proceed: PathBuf,
        attempt: PathBuf,
        would_block: PathBuf,
    }

    impl RootAuthoritySync {
        fn new(root: &Path, label: &str) -> Self {
            Self {
                gate_acquired: root.join(format!("{label}.gate-acquired")),
                proceed: root.join(format!("{label}.proceed")),
                attempt: root.join(format!("{label}.root-attempt")),
                would_block: root.join(format!("{label}.root-would-block")),
            }
        }
    }

    fn spawn_synchronized_helper(
        role: &str,
        root: &Path,
        ready: &Path,
        release: &Path,
        sync: &RootAuthoritySync,
    ) -> Child {
        Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("producer_quiescence::tests::subprocess_entrypoint")
            .arg("--nocapture")
            .env("FORGE_BOUNDARY_HELPER_ROLE", role)
            .env("FORGE_BOUNDARY_HELPER_ROOT", root)
            .env("FORGE_BOUNDARY_HELPER_READY", ready)
            .env("FORGE_BOUNDARY_HELPER_RELEASE", release)
            .env(TEST_ROOT_AUTHORITY_GATE_ACQUIRED, &sync.gate_acquired)
            .env(TEST_ROOT_AUTHORITY_PROCEED, &sync.proceed)
            .env(TEST_ROOT_AUTHORITY_ATTEMPT, &sync.attempt)
            .env(TEST_ROOT_AUTHORITY_WOULD_BLOCK, &sync.would_block)
            .spawn()
            .expect("spawn synchronized boundary helper")
    }

    #[test]
    fn subprocess_entrypoint() {
        let Ok(role) = std::env::var("FORGE_BOUNDARY_HELPER_ROLE") else {
            return;
        };
        let root = PathBuf::from(std::env::var_os("FORGE_BOUNDARY_HELPER_ROOT").expect("root"));
        let ready = PathBuf::from(std::env::var_os("FORGE_BOUNDARY_HELPER_READY").expect("ready"));
        let release =
            PathBuf::from(std::env::var_os("FORGE_BOUNDARY_HELPER_RELEASE").expect("release"));
        let cancellation = AtomicBool::new(false);
        match role.as_str() {
            "producer" => {
                let _guard = admit_producer(&root).expect("child producer admission");
                fs::write(&ready, b"ready").expect("publish helper readiness");
                wait_for(&release);
            }
            "producer-expect-root-replaced" => {
                let guard = admit_producer(&root).expect("child producer admission");
                fs::write(&ready, b"ready").expect("publish helper readiness");
                wait_for(&release);
                let rejected = crate::acquire_effect_store_lock_under_boundary(
                    &guard,
                    &root,
                    "locks/replaced-root-validation.lock",
                )
                .expect_err("old producer authority crossed into replacement root");
                assert!(matches!(
                    rejected,
                    crate::EffectStoreLockError::ProducerBoundary { .. }
                ));
            }
            "quiescer" => {
                let _guard =
                    quiesce_host_producers(&root, &cancellation).expect("child host quiescence");
                fs::write(&ready, b"ready").expect("publish helper readiness");
                wait_for(&release);
            }
            "quiescer-expect-root-replaced" => {
                let guard =
                    quiesce_host_producers(&root, &cancellation).expect("child host quiescence");
                fs::write(&ready, b"ready").expect("publish helper readiness");
                wait_for(&release);
                let rejected = crate::acquire_effect_store_lock_under_boundary(
                    &guard,
                    &root,
                    "locks/replaced-root-validation.lock",
                )
                .expect_err("replacement-root quiescence crossed into restored root");
                assert!(matches!(
                    rejected,
                    crate::EffectStoreLockError::ProducerBoundary { .. }
                ));
            }
            "effect-a" => {
                let _lock = crate::acquire_effect_store_lock(&root, "locks/a.lock")
                    .expect("child effect authority");
                fs::write(&ready, b"ready").expect("publish helper readiness");
                wait_for(&release);
            }
            "effect-b-blocking" => {
                let _lock = crate::acquire_effect_store_lock(&root, "locks/b.lock")
                    .expect("child effect authority");
                fs::write(root.join("effect-b.acquired"), b"acquired")
                    .expect("publish helper acquisition");
                wait_for(&release);
            }
            "effect-b-gate-blocking" => {
                fs::write(&ready, b"attempting").expect("publish helper lock attempt");
                let _lock = crate::acquire_effect_store_lock(&root, "locks/b.lock")
                    .expect("child effect authority");
                fs::write(root.join("effect-b.acquired"), b"acquired")
                    .expect("publish helper acquisition");
                wait_for(&release);
            }
            "effect-b-try" => {
                assert!(matches!(
                    crate::try_acquire_effect_store_lock(&root, "locks/b.lock"),
                    Err(crate::EffectStoreLockError::WouldBlock { .. })
                ));
                fs::write(&ready, b"blocked").expect("publish helper blocked result");
            }
            "effect-under-quiescence" => {
                let host =
                    quiesce_host_producers(&root, &cancellation).expect("child host quiescence");
                let _lock =
                    crate::acquire_effect_store_lock_under_boundary(&host, &root, "locks/a.lock")
                        .expect("effect authority under host quiescence");
                drop(host);
                fs::write(&ready, b"ready").expect("publish helper readiness");
                wait_for(&release);
            }
            _ => panic!("unknown helper role {role}"),
        }
    }

    #[test]
    fn local_reentrancy_last_release_and_forbidden_upgrades() {
        let root = TestRoot::new("reentrant");
        let first = admit_producer(&root.0).expect("first admission");
        let threaded_root = root.0.clone();
        let second = std::thread::spawn(move || admit_producer(threaded_root).expect("nested"))
            .join()
            .expect("thread");
        assert!(matches!(
            quiesce_host_producers(&root.0, &AtomicBool::new(false)),
            Err(ProducerBoundaryError::ReentrantUpgrade { .. })
        ));
        drop(first);
        assert!(matches!(
            quiesce_host_producers(&root.0, &AtomicBool::new(false)),
            Err(ProducerBoundaryError::ReentrantUpgrade { .. })
        ));
        drop(second);
        let exclusive = quiesce_host_producers(&root.0, &AtomicBool::new(false))
            .expect("exclusive after final shared release");
        assert!(matches!(
            admit_producer(&root.0),
            Err(ProducerBoundaryError::Quiescing { .. })
        ));
        assert!(matches!(
            quiesce_host_producers(&root.0, &AtomicBool::new(false)),
            Err(ProducerBoundaryError::NestedExclusive { .. })
        ));
        drop(exclusive);
        admit_producer(&root.0).expect("admission after exclusive release");
    }

    #[test]
    fn typed_under_boundary_authority_retains_exclusive_lease() {
        let root = TestRoot::new("typed-under-boundary");
        let shared = admit_producer(&root.0).expect("shared producer boundary");
        let rejected = crate::acquire_effect_store_lock_under_boundary(
            &shared,
            &root.0,
            "locks/test-authority.lock",
        )
        .expect_err("shared producer lease must not self-upgrade");
        assert!(matches!(
            rejected,
            crate::EffectStoreLockError::ProducerBoundary {
                source: ProducerBoundaryError::ReentrantUpgrade { .. }
            }
        ));
        drop(shared);
        let exclusive =
            quiesce_host_producers(&root.0, &AtomicBool::new(false)).expect("host quiescence");
        let authority = crate::acquire_effect_store_lock_under_boundary(
            &exclusive,
            &root.0,
            "locks/test-authority.lock",
        )
        .expect("authority under typed boundary");
        drop(exclusive);
        assert!(matches!(
            admit_producer(&root.0),
            Err(ProducerBoundaryError::Quiescing { .. })
        ));
        drop(authority);
        admit_producer(&root.0).expect("authority drop released retained boundary");
        let effect = crate::acquire_effect_store_lock(&root.0, "locks/effect-owner.lock")
            .expect("effect authority");
        let nested_producer = admit_producer(&root.0).expect("producer under effect authority");
        let rejected = crate::acquire_effect_store_lock_under_boundary(
            &nested_producer,
            &root.0,
            "locks/nested-effect.lock",
        )
        .expect_err("logical shared lease under effect must not promote");
        assert!(matches!(
            rejected,
            crate::EffectStoreLockError::ProducerBoundary {
                source: ProducerBoundaryError::ReentrantUpgrade { .. }
            }
        ));
        drop(nested_producer);
        drop(effect);
    }

    #[test]
    fn cancellation_and_deadline_release_partial_locks() {
        let root = TestRoot::new("cancel-deadline");
        drop(admit_producer(&root.0).expect("initialize lock files"));
        let drain_path = root.0.join(PRODUCER_DRAIN_LOCK);
        let drain = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&drain_path)
            .expect("drain");
        FileExt::lock_shared(&drain).expect("hold drain shared");
        let cancellation = Arc::new(AtomicBool::new(false));
        let thread_root = root.0.clone();
        let thread_cancel = Arc::clone(&cancellation);
        let waiter =
            std::thread::spawn(move || quiesce_host_producers(thread_root, &thread_cancel));
        std::thread::sleep(Duration::from_millis(80));
        cancellation.store(true, Ordering::Release);
        let result = waiter.join().expect("quiescer thread");
        assert!(matches!(
            result,
            Err(ProducerBoundaryError::Cancelled { .. })
        ));
        FileExt::unlock(&drain).expect("release raw drain");
        admit_producer(&root.0).expect("cancelled quiescer released gate");

        let gate_path = root.0.join(PRODUCER_GATE_LOCK);
        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&gate_path)
            .expect("gate");
        FileExt::lock(&gate).expect("hold gate");
        let pin = pin_state_root(&root.0).expect("pin root");
        let deadline_result = admit_pinned(
            pin,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_millis(60),
        );
        assert!(matches!(
            deadline_result,
            Err(ProducerBoundaryError::DeadlineExceeded { .. })
        ));
        FileExt::unlock(&gate).expect("release raw gate");
        admit_producer(&root.0).expect("deadline released registry slot");
    }

    #[test]
    fn subprocess_distinct_effect_paths_contend_on_root_authority() {
        let root = TestRoot::new("effect-root-authority");
        let holder_release = root.0.join("effect-a.release");
        let holder_ready = root.0.join("effect-a.ready");
        let mut holder = spawn_helper("effect-a", &root.0, &holder_ready, &holder_release);
        wait_for(&holder_ready);

        let contender_release = root.0.join("effect-b.release");
        let sync = RootAuthoritySync::new(&root.0, "effect-contender");
        let mut contender = spawn_synchronized_helper(
            "effect-b-blocking",
            &root.0,
            &root.0.join("unused.ready"),
            &contender_release,
            &sync,
        );
        wait_for(&sync.gate_acquired);
        fs::write(&sync.proceed, b"proceed").expect("release effect contender root attempt");
        wait_for(&sync.attempt);
        wait_for(&sync.would_block);
        let acquired = root.0.join("effect-b.acquired");
        assert!(
            !acquired.exists(),
            "effect contender acquired before the holder released root authority"
        );

        fs::write(&holder_release, b"release").expect("release effect holder");
        assert!(holder.wait().expect("effect holder").success());
        wait_for(&acquired);
        fs::write(&contender_release, b"release").expect("release effect contender");
        assert!(contender.wait().expect("effect contender").success());
    }

    #[test]
    fn subprocess_effect_holder_blocks_host_quiescence_at_root_authority() {
        let root = TestRoot::new("effect-host-root-authority");
        let holder_release = root.0.join("effect-a.release");
        let holder_ready = root.0.join("effect-a.ready");
        let mut holder = spawn_helper("effect-a", &root.0, &holder_ready, &holder_release);
        wait_for(&holder_ready);

        let contender_release = root.0.join("host-quiescence.release");
        let contender_ready = root.0.join("host-quiescence.ready");
        let sync = RootAuthoritySync::new(&root.0, "host-quiescence-contender");
        let mut contender = spawn_synchronized_helper(
            "quiescer",
            &root.0,
            &contender_ready,
            &contender_release,
            &sync,
        );
        wait_for(&sync.gate_acquired);
        fs::write(&sync.proceed, b"proceed")
            .expect("release host-quiescence contender root attempt");
        wait_for(&sync.attempt);
        wait_for(&sync.would_block);
        assert!(
            !contender_ready.exists(),
            "host quiescence acquired before the holder released root authority"
        );

        fs::write(&holder_release, b"release").expect("release effect holder");
        assert!(holder.wait().expect("effect holder").success());
        wait_for(&contender_ready);
        fs::write(&contender_release, b"release").expect("release host quiescence contender");
        assert!(contender
            .wait()
            .expect("host quiescence contender")
            .success());
    }

    #[test]
    fn subprocess_effect_under_dropped_host_guard_retains_gate() {
        let root = TestRoot::new("host-effect-reuse");
        let holder_release = root.0.join("holder.release");
        let holder_ready = root.0.join("holder.ready");
        let mut holder = spawn_helper(
            "effect-under-quiescence",
            &root.0,
            &holder_ready,
            &holder_release,
        );
        wait_for(&holder_ready);

        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.0.join(PRODUCER_GATE_LOCK))
            .expect("gate probe");
        assert!(matches!(
            FileExt::try_lock(&gate),
            Err(TryLockError::WouldBlock)
        ));
        drop(gate);

        let contender_release = root.0.join("contender.release");
        let contender_attempt = root.0.join("contender.attempting");
        let mut contender = spawn_helper(
            "effect-b-gate-blocking",
            &root.0,
            &contender_attempt,
            &contender_release,
        );
        wait_for(&contender_attempt);
        let acquired = root.0.join("effect-b.acquired");
        assert!(
            !acquired.exists(),
            "effect lock acquired before the retained exclusive lease released its gate"
        );

        fs::write(&holder_release, b"release").expect("release effect holder");
        assert!(holder.wait().expect("effect holder").success());
        wait_for(&acquired);
        fs::write(&contender_release, b"release").expect("release effect contender");
        assert!(contender.wait().expect("effect contender").success());
    }

    #[test]
    fn subprocess_writer_preference_and_two_shared_producers() {
        let root = TestRoot::new("writer-preference");
        let producer_release = root.0.join("producer.release");
        let producer_one_ready = root.0.join("producer-one.ready");
        let producer_two_ready = root.0.join("producer-two.ready");
        let mut producer_one =
            spawn_helper("producer", &root.0, &producer_one_ready, &producer_release);
        let mut producer_two =
            spawn_helper("producer", &root.0, &producer_two_ready, &producer_release);
        wait_for(&producer_one_ready);
        wait_for(&producer_two_ready);

        let quiescer_ready = root.0.join("quiescer.ready");
        let quiescer_release = root.0.join("quiescer.release");
        let mut quiescer = spawn_helper("quiescer", &root.0, &quiescer_ready, &quiescer_release);
        let gate_path = root.0.join(PRODUCER_GATE_LOCK);
        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&gate_path)
            .expect("gate");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match FileExt::try_lock(&gate) {
                Err(TryLockError::WouldBlock) => break,
                Ok(()) => {
                    FileExt::unlock(&gate).expect("release probe");
                    assert!(Instant::now() < deadline, "quiescer never closed gate");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryLockError::Error(error)) => panic!("gate probe failed: {error}"),
            }
        }

        let late_ready = root.0.join("late.ready");
        let late_release = root.0.join("late.release");
        let mut late = spawn_helper("producer", &root.0, &late_ready, &late_release);
        std::thread::sleep(Duration::from_millis(100));
        assert!(!late_ready.exists(), "late producer crossed a closed gate");
        fs::write(&producer_release, b"release").expect("release producers");
        wait_for(&quiescer_ready);
        assert!(
            !late_ready.exists(),
            "late producer crossed held quiescence"
        );
        fs::write(&quiescer_release, b"release").expect("release quiescer");
        wait_for(&late_ready);
        fs::write(&late_release, b"release").expect("release late producer");
        assert!(producer_one.wait().expect("producer one").success());
        assert!(producer_two.wait().expect("producer two").success());
        assert!(quiescer.wait().expect("quiescer").success());
        assert!(late.wait().expect("late producer").success());
    }

    #[test]
    fn same_process_sibling_cannot_reuse_shared_lease_after_external_gate_closes() {
        let root = TestRoot::new("local-late-admission");
        let first = admit_producer(&root.0).expect("first local admission");
        let quiescer_ready = root.0.join("quiescer.ready");
        let quiescer_release = root.0.join("quiescer.release");
        let mut quiescer = spawn_helper("quiescer", &root.0, &quiescer_ready, &quiescer_release);

        let gate_path = root.0.join(PRODUCER_GATE_LOCK);
        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&gate_path)
            .expect("gate probe");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match FileExt::try_lock(&gate) {
                Err(TryLockError::WouldBlock) => break,
                Ok(()) => {
                    FileExt::unlock(&gate).expect("release probe");
                    assert!(Instant::now() < deadline, "quiescer never closed gate");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryLockError::Error(error)) => panic!("gate probe failed: {error}"),
            }
        }

        let sibling_root = root.0.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let sibling = std::thread::spawn(move || {
            sender
                .send(admit_producer(sibling_root))
                .expect("send sibling result");
        });
        let rejected = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("sibling admission returned instead of deadlocking")
            .expect_err("same-process sibling crossed the externally closed gate");
        assert!(matches!(rejected, ProducerBoundaryError::Quiescing { .. }));
        sibling.join().expect("sibling thread");

        drop(first);
        wait_for(&quiescer_ready);
        fs::write(&quiescer_release, b"release").expect("release quiescer");
        assert!(quiescer.wait().expect("quiescer").success());
        admit_producer(&root.0).expect("fresh admission after quiescence");
    }

    #[test]
    fn closed_external_gate_rejects_both_process_local_effect_reuse_paths() {
        let root = TestRoot::new("local-late-effect-admission");
        let effect = crate::acquire_effect_store_lock(&root.0, "locks/first-effect.lock")
            .expect("first local effect authority");
        let quiescer_ready = root.0.join("quiescer.ready");
        let quiescer_release = root.0.join("quiescer.release");
        let mut quiescer = spawn_helper("quiescer", &root.0, &quiescer_ready, &quiescer_release);

        let gate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.0.join(PRODUCER_GATE_LOCK))
            .expect("gate probe");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match FileExt::try_lock(&gate) {
                Err(TryLockError::WouldBlock) => break,
                Ok(()) => {
                    FileExt::unlock(&gate).expect("release probe");
                    assert!(Instant::now() < deadline, "quiescer never closed gate");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryLockError::Error(error)) => panic!("gate probe failed: {error}"),
            }
        }

        assert!(matches!(
            admit_producer(&root.0),
            Err(ProducerBoundaryError::Quiescing { .. })
        ));
        assert!(matches!(
            crate::acquire_effect_store_lock(&root.0, "locks/second-effect.lock"),
            Err(crate::EffectStoreLockError::ProducerBoundary {
                source: ProducerBoundaryError::Quiescing { .. }
            })
        ));

        drop(effect);
        wait_for(&quiescer_ready);
        fs::write(&quiescer_release, b"release").expect("release quiescer");
        assert!(quiescer.wait().expect("quiescer").success());
    }

    #[cfg(unix)]
    #[test]
    fn authority_leaf_replacement_cannot_split_live_root_directory_authority() {
        let root = TestRoot::new("authority-leaf-replacement");
        let holder_ready = root.0.join("effect-a.ready");
        let holder_release = root.0.join("effect-a.release");
        let mut holder = spawn_helper("effect-a", &root.0, &holder_ready, &holder_release);
        wait_for(&holder_ready);

        let authority = root.0.join(PRODUCER_ROOT_AUTHORITY_LOCK);
        let displaced = root.0.join("authority-displaced");
        fs::remove_file(&authority).expect("unlink Unix authority leaf");
        fs::write(&authority, b"recreated").expect("recreate Unix authority leaf");
        fs::rename(&authority, &displaced).expect("rename recreated Unix authority leaf");
        fs::write(&authority, b"replacement").expect("replace Unix authority leaf");

        let effect_blocked = root.0.join("effect-b.blocked");
        let mut effect_contender = spawn_helper(
            "effect-b-try",
            &root.0,
            &effect_blocked,
            &root.0.join("unused"),
        );
        wait_for(&effect_blocked);
        assert!(effect_contender.wait().expect("effect contender").success());

        let exclusive_ready = root.0.join("quiescer.ready");
        let exclusive_release = root.0.join("quiescer.release");
        let sync = RootAuthoritySync::new(&root.0, "exclusive-contender");
        let mut exclusive = spawn_synchronized_helper(
            "quiescer",
            &root.0,
            &exclusive_ready,
            &exclusive_release,
            &sync,
        );
        wait_for(&sync.gate_acquired);
        fs::write(&sync.proceed, b"proceed").expect("release exclusive contender root attempt");
        wait_for(&sync.attempt);
        wait_for(&sync.would_block);
        assert!(
            !exclusive_ready.exists(),
            "exclusive contender acquired before the old effect holder released root authority"
        );

        fs::write(&holder_release, b"release").expect("release effect holder");
        assert!(holder.wait().expect("effect holder").success());
        wait_for(&exclusive_ready);
        fs::write(&exclusive_release, b"release").expect("release exclusive contender");
        assert!(exclusive.wait().expect("exclusive contender").success());
        crate::acquire_effect_store_lock(&root.0, "locks/after-replacement.lock")
            .expect("effect authority acquires after old holder and exclusive contender release");
    }
    #[cfg(unix)]
    #[test]
    fn external_unlink_recreate_cannot_split_live_lock_authority() {
        let root = TestRoot::new("unlink-recreate");
        let producer_ready = root.0.join("producer.ready");
        let producer_release = root.0.join("producer.release");
        let mut producer = spawn_helper("producer", &root.0, &producer_ready, &producer_release);
        wait_for(&producer_ready);

        fs::remove_file(root.0.join(PRODUCER_GATE_LOCK)).expect("unlink live gate");
        fs::remove_file(root.0.join(PRODUCER_DRAIN_LOCK)).expect("unlink live drain");

        let quiescer_ready = root.0.join("quiescer.ready");
        let quiescer_release = root.0.join("quiescer.release");
        let mut quiescer = spawn_helper("quiescer", &root.0, &quiescer_ready, &quiescer_release);
        std::thread::sleep(Duration::from_millis(150));
        assert!(
            !quiescer_ready.exists(),
            "quiescence acquired while the old producer remained admitted"
        );

        fs::write(&producer_release, b"release").expect("release old producer");
        wait_for(&quiescer_ready);
        fs::write(&quiescer_release, b"release").expect("release quiescer");
        assert!(producer.wait().expect("producer").success());
        assert!(quiescer.wait().expect("quiescer").success());
    }
    #[cfg(windows)]
    #[test]
    fn windows_authority_leaf_replacement_is_rejected_while_retained() {
        const ERROR_SHARING_VIOLATION: i32 = 32;

        let root = TestRoot::new("windows-authority-replacement");
        let holder_ready = root.0.join("effect-a.ready");
        let holder_release = root.0.join("effect-a.release");
        let mut holder = spawn_helper("effect-a", &root.0, &holder_ready, &holder_release);
        wait_for(&holder_ready);

        let authority = root.0.join(PRODUCER_ROOT_AUTHORITY_LOCK);
        let rename_error = fs::rename(&authority, root.0.join("authority-displaced"))
            .expect_err("retained authority must reject replacement rename");
        assert_eq!(
            rename_error.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION),
            "authority rename must fail with a Windows sharing violation"
        );
        let unlink_error = fs::remove_file(&authority)
            .expect_err("retained authority must reject unlink before recreation");
        assert_eq!(
            unlink_error.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION),
            "authority unlink must fail with a Windows sharing violation"
        );
        assert!(matches!(
            crate::try_acquire_effect_store_lock(&root.0, "locks/b.lock"),
            Err(crate::EffectStoreLockError::WouldBlock { .. })
        ));

        fs::write(&holder_release, b"release").expect("release effect holder");
        assert!(holder.wait().expect("effect holder").success());
        fs::remove_file(&authority)
            .expect("authority unlink succeeds after retained handle release");
        fs::write(&authority, b"recreated").expect("recreate authority after handle release");
        fs::rename(&authority, root.0.join("authority-displaced"))
            .expect("authority rename succeeds after retained handle release");
        fs::write(&authority, b"replacement").expect("replace authority after handle release");
        crate::acquire_effect_store_lock(&root.0, "locks/after-replacement.lock")
            .expect("effect authority acquires after retained handle release");
    }
    #[test]
    fn replacing_locks_directory_cannot_bypass_live_root_authority() {
        let root = TestRoot::new("locks-replacement");
        let holder_ready = root.0.join("effect-a.ready");
        let holder_release = root.0.join("effect-a.release");
        let mut holder = spawn_helper("effect-a", &root.0, &holder_ready, &holder_release);
        wait_for(&holder_ready);

        let displaced_locks = root.0.join("locks-old");
        if fs::rename(root.0.join("locks"), &displaced_locks).is_err() {
            // Filesystems that refuse this cooperative rename cannot exercise
            // replacement; correctness must not depend on its failure.
            fs::write(&holder_release, b"release").expect("release effect holder");
            assert!(holder.wait().expect("effect holder").success());
            return;
        }
        fs::create_dir(root.0.join("locks")).expect("replace locks directory");

        let blocked = root.0.join("effect-b.blocked");
        let mut contender = spawn_helper("effect-b-try", &root.0, &blocked, &root.0.join("unused"));
        wait_for(&blocked);
        assert!(contender.wait().expect("try contender").success());

        fs::write(&holder_release, b"release").expect("release effect holder");
        assert!(holder.wait().expect("effect holder").success());
        crate::acquire_effect_store_lock(&root.0, "locks/b.lock")
            .expect("replacement locks directory remains behind root authority");
    }

    #[cfg(unix)]
    #[test]
    fn replacing_entire_root_creates_distinct_boundary_and_old_guard_fails_closed() {
        let root = TestRoot::new("root-replacement");
        let producer_ready = root.0.join("producer.ready");
        let producer_release = root.0.join("producer.release");
        let mut producer = spawn_helper(
            "producer-expect-root-replaced",
            &root.0,
            &producer_ready,
            &producer_release,
        );
        wait_for(&producer_ready);

        let displaced_root = root.0.with_extension("old-root");
        let _ = fs::remove_dir_all(&displaced_root);
        fs::rename(&root.0, &displaced_root).expect("displace entire state root");
        fs::create_dir(&root.0).expect("create distinct replacement root");

        let quiescer_ready = root.0.join("quiescer.ready");
        let quiescer_release = root.0.join("quiescer.release");
        let mut quiescer = spawn_helper(
            "quiescer-expect-root-replaced",
            &root.0,
            &quiescer_ready,
            &quiescer_release,
        );
        wait_for(&quiescer_ready);

        fs::write(&producer_release, b"validate").expect("request old-guard validation");
        assert!(
            producer.wait().expect("producer validation").success(),
            "old typed authority did not reject the replacement root"
        );

        let replacement_root = root.0.with_extension("replacement-root");
        let _ = fs::remove_dir_all(&replacement_root);
        fs::rename(&root.0, &replacement_root).expect("retain replacement-root inode");
        fs::rename(&displaced_root, &root.0).expect("restore original authoritative root");
        fs::write(&quiescer_release, b"validate")
            .expect("request replacement-root quiescer validation");
        assert!(
            quiescer.wait().expect("quiescer validation").success(),
            "replacement-root quiescence crossed into the restored original root"
        );
        fs::remove_dir_all(&replacement_root).expect("remove replacement root");
    }
    #[test]
    fn subprocess_crash_releases_root_authority() {
        let root = TestRoot::new("crash-release");
        let ready = root.0.join("child.ready");
        let release = root.0.join("never.release");
        let mut child = spawn_helper("effect-a", &root.0, &ready, &release);
        wait_for(&ready);
        child.kill().expect("kill effect child");
        let _ = child.wait().expect("reap effect child");
        crate::acquire_effect_store_lock(&root.0, "locks/b.lock")
            .expect("OS released crashed root authority lock");
        quiesce_host_producers(&root.0, &AtomicBool::new(false))
            .expect("OS released crashed producer locks");
    }

    #[cfg(unix)]
    #[test]
    fn roots_symlinks_replacements_and_mismatches_fail_closed() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new("identity");
        let other = TestRoot::new("other");
        let shared = admit_producer(&root.0).expect("admit root");
        assert!(matches!(
            BoundaryLease::from_boundary(&shared, &other.0),
            Err(ProducerBoundaryError::BoundaryRootMismatch { .. })
        ));

        let alias = root.0.with_extension("symlink");
        let _ = fs::remove_file(&alias);
        symlink(&root.0, &alias).expect("create root symlink");
        assert!(matches!(
            admit_producer(&alias),
            Err(ProducerBoundaryError::UnsafeStateRoot { .. })
        ));
        fs::remove_file(&alias).expect("remove symlink");

        let displaced = root.0.with_extension("displaced");
        let _ = fs::remove_dir_all(&displaced);
        fs::rename(&root.0, &displaced).expect("displace root");
        fs::create_dir_all(&root.0).expect("replace root");
        assert!(matches!(
            admit_producer(&root.0),
            Err(ProducerBoundaryError::RootIdentityChanged { .. })
        ));
        drop(shared);
        let _ = fs::remove_dir_all(&root.0);
        fs::rename(&displaced, &root.0).expect("restore root for cleanup");
    }
}
