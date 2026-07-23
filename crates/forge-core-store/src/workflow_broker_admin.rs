//! Sealed retained Store authority for workflow-broker administration files.
//!
//! The CLI owns the strict broker/admin schemas and authorization checks. This
//! module owns only the exact operator-directory lock plus descriptor-relative,
//! bounded, crash-safe reads and replacements for the public registry and the
//! administration journal. No private key material is accepted or persisted.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::{EffectStoreLock, RetainedEffectStoreRoot};

const LOCK_RELATIVE_PATH: &str = ".workflow-broker.lock";
const REGISTRY_FILE: &str = "workflow-broker-registry.yaml";
const ADMIN_STATE_FILE: &str = "workflow-broker-admin.json";
pub const MAX_WORKFLOW_BROKER_REGISTRY_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_WORKFLOW_BROKER_ADMIN_STATE_BYTES: u64 = 8 * 1024 * 1024;

/// Exact retained bytes observed under the workflow-broker Store lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowBrokerStoredFile {
    bytes: Vec<u8>,
    raw_sha256: String,
}

impl WorkflowBrokerStoredFile {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn raw_sha256(&self) -> &str {
        &self.raw_sha256
    }
}

/// Store-owned authority for the exact external operator directory.
///
/// The retained producer boundary and lock stay held until this value is
/// dropped. Callers cannot select arbitrary target names through this API.
#[derive(Debug)]
pub struct WorkflowBrokerAdminStore {
    root: PathBuf,
    lock: EffectStoreLock,
}

impl WorkflowBrokerAdminStore {
    /// Acquire the exact operator-directory producer boundary and recover any
    /// interrupted registry or admin-journal replacement before returning.
    pub fn open(root: &Path) -> Result<Self, WorkflowBrokerAdminStoreError> {
        std::fs::create_dir_all(root).map_err(|source| WorkflowBrokerAdminStoreError::Io {
            path: root.to_path_buf(),
            source: source.to_string(),
        })?;
        let retained = RetainedEffectStoreRoot::acquire(root).map_err(|source| {
            WorkflowBrokerAdminStoreError::Store {
                operation: "retain operator directory",
                source: source.to_string(),
            }
        })?;
        let lock = retained
            .acquire_effect_store_lock(LOCK_RELATIVE_PATH)
            .map_err(|source| WorkflowBrokerAdminStoreError::Store {
                operation: "acquire workflow broker lock",
                source: source.to_string(),
            })?;
        let store = Self {
            root: root.to_path_buf(),
            lock,
        };
        store.recover()?;
        Ok(store)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reconcile only Store-created crash-replacement sidecars. Unknown,
    /// malformed, or substituted state fails closed in the retained protocol.
    pub fn recover(&self) -> Result<(), WorkflowBrokerAdminStoreError> {
        let io = self.lock.retained_store_io().map_err(|source| {
            WorkflowBrokerAdminStoreError::Store {
                operation: "retain workflow broker I/O",
                source: source.to_string(),
            }
        })?;
        io.recover_file_crash_safe(
            Path::new(ADMIN_STATE_FILE),
            MAX_WORKFLOW_BROKER_ADMIN_STATE_BYTES,
        )
        .map_err(|source| WorkflowBrokerAdminStoreError::Store {
            operation: "recover workflow broker admin journal",
            source: source.to_string(),
        })?;
        io.recover_file_crash_safe(Path::new(REGISTRY_FILE), MAX_WORKFLOW_BROKER_REGISTRY_BYTES)
            .map_err(|source| WorkflowBrokerAdminStoreError::Store {
                operation: "recover workflow broker registry",
                source: source.to_string(),
            })?;
        Ok(())
    }

    pub fn read_registry(
        &self,
    ) -> Result<Option<WorkflowBrokerStoredFile>, WorkflowBrokerAdminStoreError> {
        self.read_file(
            Path::new(REGISTRY_FILE),
            MAX_WORKFLOW_BROKER_REGISTRY_BYTES,
            "read workflow broker registry",
        )
    }

    pub fn read_admin_state(
        &self,
    ) -> Result<Option<WorkflowBrokerStoredFile>, WorkflowBrokerAdminStoreError> {
        self.read_file(
            Path::new(ADMIN_STATE_FILE),
            MAX_WORKFLOW_BROKER_ADMIN_STATE_BYTES,
            "read workflow broker admin journal",
        )
    }

    pub fn replace_registry(
        &self,
        expected_raw_sha256: Option<&str>,
        content: &[u8],
    ) -> Result<WorkflowBrokerStoredFile, WorkflowBrokerAdminStoreError> {
        self.replace_file(
            Path::new(REGISTRY_FILE),
            expected_raw_sha256,
            content,
            MAX_WORKFLOW_BROKER_REGISTRY_BYTES,
            "replace workflow broker registry",
        )
    }

    pub fn replace_admin_state(
        &self,
        expected_raw_sha256: Option<&str>,
        content: &[u8],
    ) -> Result<WorkflowBrokerStoredFile, WorkflowBrokerAdminStoreError> {
        self.replace_file(
            Path::new(ADMIN_STATE_FILE),
            expected_raw_sha256,
            content,
            MAX_WORKFLOW_BROKER_ADMIN_STATE_BYTES,
            "replace workflow broker admin journal",
        )
    }

    fn read_file(
        &self,
        relative: &Path,
        maximum: u64,
        operation: &'static str,
    ) -> Result<Option<WorkflowBrokerStoredFile>, WorkflowBrokerAdminStoreError> {
        let io = self.lock.retained_store_io().map_err(|source| {
            WorkflowBrokerAdminStoreError::Store {
                operation,
                source: source.to_string(),
            }
        })?;
        let witness = io
            .read_optional_bounded(relative, maximum)
            .map_err(|source| WorkflowBrokerAdminStoreError::Store {
                operation,
                source: source.to_string(),
            })?;
        Ok(witness.map(|witness| WorkflowBrokerStoredFile {
            bytes: witness.raw_bytes().to_vec(),
            raw_sha256: witness.digest().to_owned(),
        }))
    }

    fn replace_file(
        &self,
        relative: &Path,
        expected_raw_sha256: Option<&str>,
        content: &[u8],
        maximum: u64,
        operation: &'static str,
    ) -> Result<WorkflowBrokerStoredFile, WorkflowBrokerAdminStoreError> {
        if u64::try_from(content.len()).unwrap_or(u64::MAX) > maximum {
            return Err(WorkflowBrokerAdminStoreError::SizeLimit {
                path: self.root.join(relative),
                found: u64::try_from(content.len()).unwrap_or(u64::MAX),
                maximum,
            });
        }
        let io = self.lock.retained_store_io().map_err(|source| {
            WorkflowBrokerAdminStoreError::Store {
                operation,
                source: source.to_string(),
            }
        })?;
        let mut current = io
            .retain_file_crash_safe_expected_leaf(relative, maximum)
            .map_err(|source| WorkflowBrokerAdminStoreError::Store {
                operation,
                source: source.to_string(),
            })?;
        let actual_raw_sha256 = current.digest().map(str::to_owned);
        if actual_raw_sha256.as_deref() != expected_raw_sha256 {
            return Err(WorkflowBrokerAdminStoreError::CompareAndSwap {
                path: self.root.join(relative),
                expected: expected_raw_sha256.map(str::to_owned),
                actual: actual_raw_sha256,
            });
        }
        if current.raw_bytes().is_some_and(|bytes| bytes == content) {
            let raw_sha256 =
                actual_raw_sha256.ok_or_else(|| WorkflowBrokerAdminStoreError::Store {
                    operation,
                    source: "retained present bytes have no exact digest witness".to_owned(),
                })?;
            return Ok(WorkflowBrokerStoredFile {
                bytes: content.to_vec(),
                raw_sha256,
            });
        }
        let installed = io
            .replace_file_crash_safe(relative, &mut current, content, maximum)
            .map_err(|source| WorkflowBrokerAdminStoreError::Store {
                operation,
                source: source.to_string(),
            })?;
        Ok(WorkflowBrokerStoredFile {
            bytes: installed.raw_bytes().to_vec(),
            raw_sha256: installed.digest().to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowBrokerAdminStoreError {
    Io {
        path: PathBuf,
        source: String,
    },
    Store {
        operation: &'static str,
        source: String,
    },
    SizeLimit {
        path: PathBuf,
        found: u64,
        maximum: u64,
    },
    CompareAndSwap {
        path: PathBuf,
        expected: Option<String>,
        actual: Option<String>,
    },
}

impl fmt::Display for WorkflowBrokerAdminStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Store { operation, source } => write!(formatter, "{operation}: {source}"),
            Self::SizeLimit {
                path,
                found,
                maximum,
            } => write!(
                formatter,
                "{} exceeds workflow broker Store limit: {found} > {maximum}",
                path.display()
            ),
            Self::CompareAndSwap {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "workflow broker Store CAS mismatch at {}: expected {expected:?}, actual {actual:?}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for WorkflowBrokerAdminStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-workflow-broker-store-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create root");
        root
    }

    #[test]
    fn registry_and_admin_state_use_exact_crash_safe_replacement() {
        let root = temp_root("replace");
        let store = WorkflowBrokerAdminStore::open(&root).expect("open store");
        let first = store
            .replace_registry(None, b"registry: one\n")
            .expect("first registry");
        let same = store
            .replace_registry(Some(first.raw_sha256()), b"registry: one\n")
            .expect("idempotent registry");
        assert_eq!(first, same);
        let second = store
            .replace_registry(Some(same.raw_sha256()), b"registry: two\n")
            .expect("second registry");
        assert_ne!(first.raw_sha256(), second.raw_sha256());
        store
            .replace_admin_state(None, br#"{"schema_version":"test"}"#)
            .expect("admin state");
        assert_eq!(
            store
                .read_registry()
                .expect("read registry")
                .expect("registry")
                .bytes(),
            b"registry: two\n"
        );
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn stale_compare_and_swap_cannot_replace_registry_or_journal() {
        let root = temp_root("stale-cas");
        let store = WorkflowBrokerAdminStore::open(&root).expect("open store");
        let registry = store
            .replace_registry(None, b"registry: one\n")
            .expect("registry");
        let journal = store
            .replace_admin_state(None, br#"{"schema_version":"one"}"#)
            .expect("journal");
        assert!(matches!(
            store.replace_registry(None, b"registry: attacker\n"),
            Err(WorkflowBrokerAdminStoreError::CompareAndSwap { .. })
        ));
        assert!(matches!(
            store.replace_admin_state(None, br#"{"schema_version":"attacker"}"#),
            Err(WorkflowBrokerAdminStoreError::CompareAndSwap { .. })
        ));
        assert_eq!(
            store
                .read_registry()
                .expect("read registry")
                .expect("registry")
                .raw_sha256(),
            registry.raw_sha256()
        );
        assert_eq!(
            store
                .read_admin_state()
                .expect("read journal")
                .expect("journal")
                .raw_sha256(),
            journal.raw_sha256()
        );
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn oversized_content_is_rejected_before_replacement() {
        let root = temp_root("limit");
        let store = WorkflowBrokerAdminStore::open(&root).expect("open store");
        let oversized =
            vec![0_u8; usize::try_from(MAX_WORKFLOW_BROKER_REGISTRY_BYTES + 1).unwrap()];
        assert!(matches!(
            store.replace_registry(None, &oversized),
            Err(WorkflowBrokerAdminStoreError::SizeLimit { .. })
        ));
        assert!(store.read_registry().expect("read registry").is_none());
        drop(store);
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
