// The WAL replay, effect application, and metadata-index query entrypoints
// are long because they materialise full typed status structs while walking
// JSON-Lines records line-by-line. Splitting them to satisfy
// `clippy::too_many_lines` would harm readability.
#![allow(
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_self
)]

use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::runtime::RuntimeKind;
use forge_core_contracts::tool_effect::{AccessMode, EffectTargetKind, ToolEffectContractDocument};
use forge_core_contracts::{
    classify_reserved_state_path, normalize_state_relative_path, RepoPath, ReservedStatePath,
    StableId,
};
use forge_core_trace::TraceEvent;
use forge_core_validate::{
    validate_tool_effect, Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedYamlDocument,
    ReferenceIndex, ReferenceKind,
};
use fs4::{FileExt, TryLockError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use tracing::instrument;
use yaml_serde::Value;

pub mod backup;
pub mod claim_wal;
pub mod crash_replace;
pub mod derive_state;
pub mod producer_quiescence;
pub mod product_lifecycle;
pub mod project_reinitialize;
pub mod replay_anchor;
pub mod replay_wal;
pub mod restore;
pub mod retained_crash_replace;
mod retained_dir;
pub mod retained_lifecycle;
pub mod retained_project_tree;
#[cfg(windows)]
mod windows_file_info;
pub mod workflow_action_replay;
pub mod workflow_broker_admin;

/// The sole authority constructor for claim state. Replays the append-only
/// claims WAL into a typed projection. See [`fn@derive_state`] for the contract.
pub use derive_state::derive_state;
pub use retained_crash_replace::{
    OwnedRetainedCrashReplaceRead, OwnedRetainedCrashReplaceSession, RetainedCrashReplaceRead,
    RetainedCrashReplaceSession,
};

const CONTRACT_DEFINITIONS: &[&str] = &[
    "contracts/assurance/assurance-case-contract-v0.yaml",
    "contracts/commands/command-contract-v0.yaml",
    "contracts/claims/claim-contract-v0.yaml",
    "contracts/completion/completion-contract-v0.yaml",
    "contracts/decisions/decision-close-contract-v0.yaml",
    "contracts/effects/tool-effect-contract-v0.yaml",
    "contracts/evals/coordination-eval-contract-v0.yaml",
    "contracts/gates/gate-contract-v0.yaml",
    "contracts/inventory/contract-family-inventory-v0.yaml",
    "contracts/operations/operation-contract-v0.yaml",
    "contracts/requests/request-contract-v0.yaml",
    "contracts/recovery/health-recovery-contract-v0.yaml",
    "contracts/runtimes/runtime-handoff-contract-v0.yaml",
    "contracts/spec/domain-pack-learning-v0.yaml",
    "contracts/spec/domain-pack-lifecycle-v0.yaml",
    "contracts/spec/domain-pack-v0.yaml",
    "contracts/spec/workflow-governance-release-v0.yaml",
    "contracts/spec/workflow-governance-release-admission-v0.yaml",
    "contracts/spec/workflow-governance-retirement-v0.yaml",
    "contracts/workflow-governance/workflow-governance-contract-v0.yaml",
];

/// Durability tier for WAL and JSONL append paths.
///
/// See ADR-0009 (`opt-in-no-sync-wal-append`) for the full rationale. The
/// default [`WalDurability::SyncOnAppend`] preserves the historical contract:
/// every append calls `sync_all` / `sync_data` before returning, so the record
/// is durable on disk by the time the caller sees `Ok`.
///
/// [`WalDurability::NoSync`] skips that `fsync`. It is intended for
/// benchmarks, integration tests, and local dev loops where the 25–50ms
/// Windows `fsync` cost per append is prohibitive and crash durability is not
/// being asserted. Using `NoSync` in production loses the un-`fsync`ed tail of
/// the WAL on power loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WalDurability {
    /// Call `sync_all` / `sync_data` on every append (the default, historical
    /// behaviour). Required for production durability.
    #[default]
    SyncOnAppend,
    /// Skip `sync_all` / `sync_data` on append. Faster (no `fsync` per
    /// record) but NOT durable on power loss. Opt-in only; see ADR-0009.
    NoSync,
}

#[derive(Debug, Clone)]
pub struct ReferenceIndexOptions {
    pub include_standard_runtime_projections: bool,
}

impl Default for ReferenceIndexOptions {
    fn default() -> Self {
        Self {
            include_standard_runtime_projections: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReferenceIndexBuilder {
    options: ReferenceIndexOptions,
    /// Repo-relative paths of contracts that are canonically known even when
    /// absent from disk (e.g. embedded in the binary). `insert_existing`
    /// registers these regardless of disk presence, so a consumer repo
    /// without a `contracts/` tree still resolves the shared definitions.
    known_embedded_refs: std::collections::HashSet<String>,
}

impl ReferenceIndexBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_options(options: ReferenceIndexOptions) -> Self {
        Self {
            options,
            known_embedded_refs: std::collections::HashSet::new(),
        }
    }

    /// Declare repo-relative paths that are canonically known even when they
    /// are not present on disk under `root` (e.g. they are embedded in the
    /// binary). The builder will register them as existing definitions so
    /// downstream reference checks pass for a consumer repo that ships no
    /// `contracts/` tree of its own.
    #[must_use]
    pub fn with_known_embedded_refs<I, S>(mut self, refs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.known_embedded_refs = refs.into_iter().map(Into::into).collect();
        self
    }

    /// Build a [`ReferenceIndex`] by scanning the repository at `root`.
    ///
    /// # Errors
    ///
    /// Returns [`ReferenceIndexBuildError`] if any policy file, operation
    /// fixture, contract instance, or command contract cannot be parsed.
    pub fn build(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<ReferenceIndex, ReferenceIndexBuildError> {
        let root = root.as_ref();
        let mut index = ReferenceIndex::new();

        add_contract_definitions(&mut index, root, &self.known_embedded_refs);
        add_policy_files(&mut index, root, &self.known_embedded_refs)?;
        add_operation_fixtures(&mut index, root)?;
        add_contract_instances(&mut index, root, &self.known_embedded_refs)?;
        add_command_contracts(&mut index, root)?;
        add_runtime_contracts(&mut index, root, &self.known_embedded_refs)?;
        add_runtime_state_refs(&mut index, root, &self.options, &self.known_embedded_refs);

        Ok(index)
    }
}

/// Build a [`ReferenceIndex`] using default options. See
/// [`ReferenceIndexBuilder::build`] for details.
///
/// # Errors
///
/// Returns [`ReferenceIndexBuildError`] if any policy file, operation
/// fixture, contract instance, or command contract cannot be parsed.
pub fn build_reference_index(
    root: impl AsRef<Path>,
) -> Result<ReferenceIndex, ReferenceIndexBuildError> {
    ReferenceIndexBuilder::new().build(root)
}

#[derive(Debug, Clone, Default)]
pub struct YamlDocumentCollection {
    pub documents: Vec<ParsedYamlDocument>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Default)]
pub struct KnownRepoPathsCollection {
    pub paths: HashSet<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn collect_validation_yaml_documents(root: impl AsRef<Path>) -> YamlDocumentCollection {
    let root = root.as_ref();
    let mut collection = YamlDocumentCollection::default();
    collect_yaml_documents_recursive(root, &root.join("contracts"), &mut collection);
    collect_yaml_documents_recursive(
        root,
        &root
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
        &mut collection,
    );
    collection
        .documents
        .sort_by(|left, right| left.path.cmp(&right.path));
    collection
}

pub fn collect_known_repo_paths(root: impl AsRef<Path>) -> HashSet<String> {
    collect_known_repo_paths_with_diagnostics(root).paths
}

pub fn collect_known_repo_paths_with_diagnostics(
    root: impl AsRef<Path>,
) -> KnownRepoPathsCollection {
    let root = root.as_ref();
    let mut collection = KnownRepoPathsCollection::default();
    collect_known_paths_recursive(root, &root.join("contracts"), &mut collection);
    collect_frozen_workflow_logical_refs(root, &mut collection);
    // Repository references may target any documentation or fixture path.
    // This index records existence only; Markdown loading and authority remain
    // governed separately by the exhaustive Markdown retirement allowlist.
    collect_known_paths_recursive(root, &root.join("docs"), &mut collection);
    collection
}

/// Register the historical logical names preserved by the P5d.5 frozen
/// catalog. These aliases are validation-only identities: the physical files
/// remain under evidence and this does not restore them to operational routing.
fn collect_frozen_workflow_logical_refs(root: &Path, collection: &mut KnownRepoPathsCollection) {
    let archive = root.join("contracts/evidence/workflow-retirement/legacy-catalog");
    let Ok(entries) = fs::read_dir(&archive) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            collection
                .paths
                .insert(format!("contracts/workflows/{name}"));
        }
    }
}

/// Append a single JSON record followed by a newline to a repo-relative
/// append-only log. Acquires the shared effect-store lock for the log to
/// serialize concurrent appenders, then `fsync`s the file so the record is
/// durable on return.
///
/// # Errors
///
/// Returns [`AppendJsonLineError`] if the relative path is invalid, the
/// record cannot be serialized to JSON, the directory cannot be created, the
/// lock cannot be acquired, or any I/O write/flush/sync fails.
pub fn append_json_line<T>(
    root: impl AsRef<Path>,
    relative_path: &str,
    record: &T,
) -> Result<PathBuf, AppendJsonLineError>
where
    T: Serialize,
{
    append_json_line_with_lock(
        root,
        relative_path,
        &append_json_line_lock_relative_path(relative_path),
        record,
    )
}

/// Like [`append_json_line`] but lets the caller pick the [`WalDurability`]
/// tier. See ADR-0009 for when `NoSync` is appropriate (benchmarks, tests,
/// dev) and when it is not (production).
///
/// # Errors
///
/// Forwards [`AppendJsonLineError`] from `append_json_line_with_lock_durability`.
pub fn append_json_line_with_durability<T>(
    root: impl AsRef<Path>,
    relative_path: &str,
    record: &T,
    durability: WalDurability,
) -> Result<PathBuf, AppendJsonLineError>
where
    T: Serialize,
{
    append_json_line_with_lock_durability(
        root,
        relative_path,
        &append_json_line_lock_relative_path(relative_path),
        record,
        durability,
    )
}

/// Append one JSON record while retaining the caller's exact producer boundary.
/// This path never self-admits from an ambient root.
///
/// # Errors
///
/// Returns [`AppendJsonLineError`] for boundary, path, serialization, or durable
/// descriptor-relative append failure.
pub fn append_json_line_with_durability_under_boundary<T>(
    boundary: &impl producer_quiescence::ProducerBoundary,
    root: impl AsRef<Path>,
    relative_path: &str,
    record: &T,
    durability: WalDurability,
) -> Result<PathBuf, AppendJsonLineError>
where
    T: Serialize,
{
    let root = root.as_ref();
    if let Some(reserved) = reserved_state_path(relative_path) {
        return Err(AppendJsonLineError::ReservedStatePath {
            path: relative_path.to_owned(),
            reserved,
        });
    }
    let target = resolve_safe_repo_relative(root, relative_path)?;
    let relative = normalized_effect_relative_path(relative_path).ok_or_else(|| {
        AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_owned(),
        }
    })?;
    let parent = relative
        .parent()
        .ok_or_else(|| AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_owned(),
        })?;
    let mut line = serde_json::to_vec(record).map_err(|source| AppendJsonLineError::Serialize {
        path: target.clone(),
        source: source.to_string(),
    })?;
    line.push(b'\n');

    let lock_relative_path = append_json_line_lock_relative_path(relative_path);
    let lock = acquire_effect_store_lock_under_boundary(boundary, root, &lock_relative_path)
        .map_err(|source| AppendJsonLineError::Lock {
            path: lock_relative_path,
            source: source.to_string(),
        })?;
    if !parent.as_os_str().is_empty() {
        lock.root
            .create_dir_all(parent)
            .map_err(|source| AppendJsonLineError::CreateDir {
                path: root.join(parent),
                source: source.to_string(),
            })?;
    }
    if let Some(reserved) = reserved_state_path(relative_path) {
        return Err(AppendJsonLineError::ReservedStatePath {
            path: relative_path.to_owned(),
            reserved,
        });
    }
    let mut file = lock
        .root
        .open_read_write_create(&relative)
        .map_err(|source| AppendJsonLineError::OpenFile {
            path: target.clone(),
            source: source.to_string(),
        })?;
    file.seek(SeekFrom::End(0))
        .and_then(|_| file.write_all(&line))
        .and_then(|()| file.flush())
        .map_err(|source| AppendJsonLineError::Write {
            path: target.clone(),
            source: source.to_string(),
        })?;
    if let WalDurability::SyncOnAppend = durability {
        file.sync_all()
            .map_err(|source| AppendJsonLineError::Write {
                path: target.clone(),
                source: source.to_string(),
            })?;
    }
    Ok(target)
}

fn append_json_line_with_lock<T>(
    root: impl AsRef<Path>,
    relative_path: &str,
    lock_relative_path: &str,
    record: &T,
) -> Result<PathBuf, AppendJsonLineError>
where
    T: Serialize,
{
    append_json_line_with_lock_durability(
        root,
        relative_path,
        lock_relative_path,
        record,
        WalDurability::SyncOnAppend,
    )
}

fn append_json_line_with_lock_durability<T>(
    root: impl AsRef<Path>,
    relative_path: &str,
    lock_relative_path: &str,
    record: &T,
    durability: WalDurability,
) -> Result<PathBuf, AppendJsonLineError>
where
    T: Serialize,
{
    let root = root.as_ref();
    if let Some(reserved) = reserved_state_path(relative_path) {
        return Err(AppendJsonLineError::ReservedStatePath {
            path: relative_path.to_owned(),
            reserved,
        });
    }
    let target = resolve_safe_repo_relative(root, relative_path)?;
    let relative = normalized_effect_relative_path(relative_path).ok_or_else(|| {
        AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_owned(),
        }
    })?;
    let parent = relative
        .parent()
        .ok_or_else(|| AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_string(),
        })?;
    let mut line = serde_json::to_vec(record).map_err(|source| AppendJsonLineError::Serialize {
        path: target.clone(),
        source: source.to_string(),
    })?;
    line.push(b'\n');

    let lock = acquire_effect_store_lock(root, lock_relative_path).map_err(|source| {
        AppendJsonLineError::Lock {
            path: lock_relative_path.to_string(),
            source: source.to_string(),
        }
    })?;

    if !parent.as_os_str().is_empty() {
        lock.root
            .create_dir_all(parent)
            .map_err(|source| AppendJsonLineError::CreateDir {
                path: root.join(parent),
                source: source.to_string(),
            })?;
    }
    // Recheck at the final mutation point; the caller-controlled spelling is
    // never converted into authority after preflight.
    if let Some(reserved) = reserved_state_path(relative_path) {
        return Err(AppendJsonLineError::ReservedStatePath {
            path: relative_path.to_owned(),
            reserved,
        });
    }
    let mut file = lock
        .root
        .open_read_write_create(&relative)
        .map_err(|source| AppendJsonLineError::OpenFile {
            path: target.clone(),
            source: source.to_string(),
        })?;
    file.seek(SeekFrom::End(0))
        .and_then(|_| file.write_all(&line))
        .and_then(|()| file.flush())
        .map_err(|source| AppendJsonLineError::Write {
            path: target.clone(),
            source: source.to_string(),
        })?;
    if let WalDurability::SyncOnAppend = durability {
        file.sync_all()
            .map_err(|source| AppendJsonLineError::Write {
                path: target.clone(),
                source: source.to_string(),
            })?;
    }

    Ok(target)
}

/// Append every record in `records` to a repo-relative append-only log,
/// returning the on-disk paths of the appended entries.
///
/// # Errors
///
/// Forwards [`AppendJsonLineError`] from [`append_json_line`] on the first
/// record that fails.
pub fn append_effect_target_metadata_records(
    root: impl AsRef<Path>,
    index_relative_path: &str,
    records: &[EffectTargetMetadataRecord],
) -> Result<Vec<PathBuf>, AppendJsonLineError> {
    append_effect_target_metadata_records_with_durability(
        root,
        index_relative_path,
        records,
        WalDurability::default(),
    )
}

/// [`append_effect_target_metadata_records`] with an explicit [`WalDurability`] knob.
/// See ADR-0009.
///
/// # Errors
///
/// Forwards [`AppendJsonLineError`] from [`append_json_line_with_durability`] on
/// the first record that fails.
pub fn append_effect_target_metadata_records_with_durability(
    root: impl AsRef<Path>,
    index_relative_path: &str,
    records: &[EffectTargetMetadataRecord],
    durability: WalDurability,
) -> Result<Vec<PathBuf>, AppendJsonLineError> {
    let root = root.as_ref();
    let _batch_boundary = if records.is_empty() {
        None
    } else {
        let state_root = effect_boundary_state_root(root, index_relative_path);
        Some(
            producer_quiescence::admit_effect_producer(&state_root, false).map_err(|source| {
                AppendJsonLineError::Lock {
                    path: index_relative_path.to_owned(),
                    source: source.to_string(),
                }
            })?,
        )
    };
    let mut paths = Vec::with_capacity(records.len());
    for record in records {
        paths.push(append_json_line_with_durability(
            root,
            index_relative_path,
            record,
            durability,
        )?);
    }
    Ok(paths)
}

/// Append a metadata batch through one caller-retained exact producer boundary.
///
/// # Errors
///
/// Returns the first [`AppendJsonLineError`] without minting a replacement
/// producer admission.
pub fn append_effect_target_metadata_records_with_durability_under_boundary(
    boundary: &impl producer_quiescence::ProducerBoundary,
    root: impl AsRef<Path>,
    index_relative_path: &str,
    records: &[EffectTargetMetadataRecord],
    durability: WalDurability,
) -> Result<Vec<PathBuf>, AppendJsonLineError> {
    let root = root.as_ref();
    let mut paths = Vec::with_capacity(records.len());
    for record in records {
        paths.push(append_json_line_with_durability_under_boundary(
            boundary,
            root,
            index_relative_path,
            record,
            durability,
        )?);
    }
    Ok(paths)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectTargetResolveError {
    InvalidTargetPath {
        target_kind: EffectTargetKind,
        reference: String,
        source: String,
    },
    ReservedStatePath {
        target_kind: EffectTargetKind,
        reference: String,
        reserved: ReservedStatePath,
    },
}

impl fmt::Display for EffectTargetResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTargetPath {
                target_kind,
                reference,
                source,
            } => write!(
                formatter,
                "resolve effect target {target_kind:?} {reference} failed: {source}"
            ),
            Self::ReservedStatePath {
                target_kind,
                reference,
                reserved,
            } => write!(
                formatter,
                "effect target {target_kind:?} {reference} is reserved for EventLog TCB: {reserved:?}"
            ),
        }
    }
}

impl std::error::Error for EffectTargetResolveError {}

/// Resolve a `ToolEffect` target to the repo-relative physical path used by the
/// effect store when applying file-backed writes.
///
/// # Errors
///
/// Returns [`EffectTargetResolveError::InvalidTargetPath`] when the target kind
/// is unsupported for file-backed effect application or when the resolved path
/// escapes the supplied root.
pub fn resolve_effect_physical_ref(
    root: impl AsRef<Path>,
    target_kind: EffectTargetKind,
    reference: &str,
) -> Result<RepoPath, EffectTargetResolveError> {
    resolve_effect_target(root.as_ref(), target_kind, reference)
        .map(|(_path, physical_ref)| RepoPath(physical_ref))
        .map_err(|source| match source {
            AppendJsonLineError::ReservedStatePath { reserved, .. } => {
                EffectTargetResolveError::ReservedStatePath {
                    target_kind,
                    reference: reference.to_owned(),
                    reserved,
                }
            }
            source => EffectTargetResolveError::InvalidTargetPath {
                target_kind,
                reference: reference.to_owned(),
                source: source.to_string(),
            },
        })
}

pub const DEFAULT_TRACE_LOG_RELATIVE_PATH: &str = "traces/events.ndjson";

/// Append a single [`TraceEvent`] to the default trace log
/// ([`DEFAULT_TRACE_LOG_RELATIVE_PATH`]) and `fsync` it for durability.
///
/// # Errors
///
/// Forwards [`AppendJsonLineError`] from the underlying append routine.
pub fn append_trace_event(
    state_root: impl AsRef<Path>,
    event: &TraceEvent,
) -> Result<PathBuf, AppendJsonLineError> {
    append_json_line_with_lock(
        state_root,
        DEFAULT_TRACE_LOG_RELATIVE_PATH,
        &state_append_json_line_lock_relative_path(DEFAULT_TRACE_LOG_RELATIVE_PATH),
        event,
    )
}

/// Append one trace event through the operation's retained exact producer
/// boundary without admitting a second producer.
///
/// # Errors
///
/// Returns [`AppendJsonLineError`] for boundary, path, serialization, or durable
/// descriptor-relative append failure.
pub fn append_trace_event_under_boundary(
    boundary: &impl producer_quiescence::ProducerBoundary,
    state_root: impl AsRef<Path>,
    event: &TraceEvent,
) -> Result<PathBuf, AppendJsonLineError> {
    let state_root = state_root.as_ref();
    let relative_path = DEFAULT_TRACE_LOG_RELATIVE_PATH;
    let target = resolve_safe_repo_relative(state_root, relative_path)?;
    let relative = normalized_effect_relative_path(relative_path).ok_or_else(|| {
        AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_owned(),
        }
    })?;
    let parent = relative
        .parent()
        .ok_or_else(|| AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_owned(),
        })?;
    let mut line = serde_json::to_vec(event).map_err(|source| AppendJsonLineError::Serialize {
        path: target.clone(),
        source: source.to_string(),
    })?;
    line.push(b'\n');
    let lock_relative_path = state_append_json_line_lock_relative_path(relative_path);
    let lock = acquire_effect_store_lock_under_boundary(boundary, state_root, &lock_relative_path)
        .map_err(|source| AppendJsonLineError::Lock {
            path: lock_relative_path,
            source: source.to_string(),
        })?;
    if !parent.as_os_str().is_empty() {
        lock.root
            .create_dir_all(parent)
            .map_err(|source| AppendJsonLineError::CreateDir {
                path: state_root.join(parent),
                source: source.to_string(),
            })?;
    }
    let mut file = lock
        .root
        .open_read_write_create(&relative)
        .map_err(|source| AppendJsonLineError::OpenFile {
            path: target.clone(),
            source: source.to_string(),
        })?;
    file.seek(SeekFrom::End(0))
        .and_then(|_| file.write_all(&line))
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| AppendJsonLineError::Write {
            path: target.clone(),
            source: source.to_string(),
        })?;
    Ok(target)
}

pub fn query_trace_events(
    state_root: impl AsRef<Path>,
    query: &TraceEventQuery,
) -> TraceEventQueryResult {
    query_trace_events_at(state_root, DEFAULT_TRACE_LOG_RELATIVE_PATH, query)
}

pub fn query_trace_events_at(
    state_root: impl AsRef<Path>,
    trace_relative_path: &str,
    query: &TraceEventQuery,
) -> TraceEventQueryResult {
    let state_root = state_root.as_ref();
    let Ok(trace_path) = resolve_safe_repo_relative(state_root, trace_relative_path) else {
        return TraceEventQueryResult {
            status: TraceEventQueryStatus::Failed,
            scanned_events: 0,
            matched_events: 0,
            returned_events: 0,
            events: Vec::new(),
            reasons: vec![TraceEventQueryReason::InvalidTracePath],
            diagnostics: vec![format!("invalid trace path {trace_relative_path}")],
        };
    };
    if !trace_path.exists() {
        return TraceEventQueryResult {
            status: TraceEventQueryStatus::Noop,
            scanned_events: 0,
            matched_events: 0,
            returned_events: 0,
            events: Vec::new(),
            reasons: vec![TraceEventQueryReason::NoTraceFile],
            diagnostics: Vec::new(),
        };
    }
    let text = match fs::read_to_string(&trace_path) {
        Ok(text) => text,
        Err(error) => {
            return TraceEventQueryResult {
                status: TraceEventQueryStatus::Failed,
                scanned_events: 0,
                matched_events: 0,
                returned_events: 0,
                events: Vec::new(),
                reasons: vec![TraceEventQueryReason::TraceReadFailed],
                diagnostics: vec![format!("read trace log failed: {error}")],
            };
        }
    };

    let mut scanned_events = 0usize;
    let mut matched = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let event = match serde_json::from_str::<TraceEvent>(line) {
            Ok(event) => event,
            Err(error) => {
                return TraceEventQueryResult {
                    status: TraceEventQueryStatus::Failed,
                    scanned_events,
                    matched_events: matched.len(),
                    returned_events: 0,
                    events: Vec::new(),
                    reasons: vec![TraceEventQueryReason::TraceParseFailed],
                    diagnostics: vec![format!(
                        "parse trace log line {} failed: {error}",
                        index + 1
                    )],
                };
            }
        };
        scanned_events += 1;
        if trace_event_matches(&event, query) {
            matched.push(event);
        }
    }

    if query.latest_run {
        if let Some(run_id) = matched.last().map(|event| event.run_id.clone()) {
            matched.retain(|event| event.run_id == run_id);
        }
    }
    if let Some(limit) = query.limit {
        if matched.len() > limit {
            let keep_from = matched.len() - limit;
            matched = matched.split_off(keep_from);
        }
    }

    let returned_events = matched.len();
    TraceEventQueryResult {
        status: TraceEventQueryStatus::Matched,
        scanned_events,
        matched_events: returned_events,
        returned_events,
        events: matched,
        reasons: vec![TraceEventQueryReason::Matched],
        diagnostics: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TraceEventQuery {
    pub run_id: Option<String>,
    pub trace_id: Option<String>,
    pub latest_run: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEventQueryResult {
    pub status: TraceEventQueryStatus,
    pub scanned_events: usize,
    pub matched_events: usize,
    pub returned_events: usize,
    pub events: Vec<TraceEvent>,
    pub reasons: Vec<TraceEventQueryReason>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventQueryStatus {
    Noop,
    Matched,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventQueryReason {
    InvalidTracePath,
    NoTraceFile,
    TraceReadFailed,
    TraceParseFailed,
    Matched,
}

fn trace_event_matches(event: &TraceEvent, query: &TraceEventQuery) -> bool {
    if let Some(run_id) = &query.run_id {
        if &event.run_id != run_id {
            return false;
        }
    }
    if let Some(trace_id) = &query.trace_id {
        if &event.trace_id != trace_id {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectApplicationPayload {
    pub target_ref: String,
    pub content: Vec<u8>,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectApplicationResult {
    pub status: EffectApplicationStatus,
    pub effect_id: StableId,
    pub applied_refs: Vec<String>,
    pub metadata_records: Vec<EffectTargetMetadataRecord>,
    pub rolled_back: bool,
    pub reasons: Vec<EffectApplicationReason>,
    pub diagnostics: Vec<String>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectTargetMetadataRecord {
    pub schema_version: String,
    pub record_kind: EffectTargetMetadataRecordKind,
    #[serde(default)]
    pub recorded_at: Option<String>,
    pub operation_id: StableId,
    pub effect_id: StableId,
    pub logical_ref: String,
    pub physical_ref: String,
    pub target_kind: EffectTargetKind,
    pub access_mode: AccessMode,
    pub content_hash: Option<String>,
    pub byte_len: u64,
    pub actor_agent_id: StableId,
    pub actor_role: ActorRole,
    pub destructive: bool,
    pub redaction_hint: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTargetMetadataRecordKind {
    EffectTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectApplicationStatus {
    Applied,
    Blocked,
    RolledBack,
    RollbackFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectApplicationReason {
    EffectValidationErrors,
    InternalInvariant,
    StoreLockFailed,
    ExecutionProvenanceInvalid,
    WalAppendFailed,
    UnsupportedTargetKind,
    UnsupportedAccessMode,
    InvalidTargetPath,
    ReservedStatePath,
    MissingPayloadForWrite,
    PayloadHashMismatch,
    MissingExpectedHashForOverwrite,
    ExpectedHashMismatch,
    TargetExistsForCreate,
    TargetMissingForWrite,
    TargetMissingForDelete,
    ApplyFailed,
    RollbackFailed,
    Applied,
}

/// Read-only effect preflight captured while the caller retains the exact
/// effect-store lock. It proves that contract validation, read freshness,
/// payload hashes, target scope, and write revalidation all completed before
/// any effect-WAL record was appended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct EffectPreflightResult {
    pub status: EffectPreflightStatus,
    pub effect_id: StableId,
    pub metadata_records: Vec<EffectTargetMetadataRecord>,
    pub reasons: Vec<EffectApplicationReason>,
    pub diagnostics: Vec<String>,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EffectPreflightStatus {
    Ready,
    Blocked,
}

/// Schema for the complete kernel-owned authority and Admission evidence
/// embedded in the first record of a prepared effect transaction.
pub const EFFECT_EXECUTION_PROVENANCE_SCHEMA_VERSION: &str = "0.1";

/// Canonical, content-addressed execution evidence persisted before the first
/// project write. The store treats the document as opaque kernel evidence but
/// verifies its canonical digest both on append and during reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct EffectExecutionProvenance {
    pub schema_version: String,
    pub digest: String,
    pub document: serde_json::Value,
}

impl EffectExecutionProvenance {
    /// Build a content-addressed provenance envelope from a typed kernel JSON
    /// projection.
    ///
    /// # Errors
    ///
    /// Returns [`EffectExecutionProvenanceError`] when canonical JSON encoding
    /// fails.
    pub fn new(document: serde_json::Value) -> Result<Self, EffectExecutionProvenanceError> {
        let canonical = serde_json_canonicalizer::to_vec(&document)
            .map_err(|error| EffectExecutionProvenanceError::Canonicalization(error.to_string()))?;
        Ok(Self {
            schema_version: EFFECT_EXECUTION_PROVENANCE_SCHEMA_VERSION.to_owned(),
            digest: sha256_content_hash(&canonical),
            document,
        })
    }

    /// Recompute and verify the canonical provenance digest.
    ///
    /// # Errors
    ///
    /// Returns [`EffectExecutionProvenanceError`] for an unsupported schema,
    /// failed canonicalization, or digest mismatch.
    pub fn verify(&self) -> Result<(), EffectExecutionProvenanceError> {
        if self.schema_version != EFFECT_EXECUTION_PROVENANCE_SCHEMA_VERSION {
            return Err(EffectExecutionProvenanceError::UnsupportedSchemaVersion {
                found: self.schema_version.clone(),
            });
        }
        let canonical = serde_json_canonicalizer::to_vec(&self.document)
            .map_err(|error| EffectExecutionProvenanceError::Canonicalization(error.to_string()))?;
        let actual = sha256_content_hash(&canonical);
        if actual != self.digest {
            return Err(EffectExecutionProvenanceError::DigestMismatch {
                expected: self.digest.clone(),
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EffectExecutionProvenanceError {
    UnsupportedSchemaVersion { found: String },
    Canonicalization(String),
    DigestMismatch { expected: String, actual: String },
}

impl fmt::Display for EffectExecutionProvenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { found } => {
                write!(formatter, "unsupported execution provenance schema {found}")
            }
            Self::Canonicalization(error) => {
                write!(
                    formatter,
                    "execution provenance canonicalization failed: {error}"
                )
            }
            Self::DigestMismatch { expected, actual } => write!(
                formatter,
                "execution provenance digest mismatch: expected {expected}, actual {actual}"
            ),
        }
    }
}

impl std::error::Error for EffectExecutionProvenanceError {}

/// Replay reservation identity bound into the effect WAL without persisting
/// the raw nonce, principal id, or audience.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct EffectReplayCommitBinding {
    pub key_hash: String,
    pub intent_digest: String,
    pub commit_digest: String,
    pub reservation_revision: u64,
}

impl EffectReplayCommitBinding {
    #[must_use]
    pub fn new(
        key_hash: impl Into<String>,
        intent_digest: impl Into<String>,
        commit_digest: impl Into<String>,
        reservation_revision: u64,
    ) -> Self {
        Self {
            key_hash: key_hash.into(),
            intent_digest: intent_digest.into(),
            commit_digest: commit_digest.into(),
            reservation_revision,
        }
    }
}

/// Durable evidence that the replay transition corresponding to one committed
/// effect reached `consumed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct EffectReplayCompletion {
    pub key_hash: String,
    pub reservation_revision: u64,
    pub consumed_revision: u64,
    pub consumed_seq: u64,
    pub recovered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectWalRecord {
    pub schema_version: String,
    pub tx_id: String,
    pub stage: EffectWalStage,
    pub effect_id: StableId,
    pub target_ref: Option<String>,
    #[serde(default)]
    pub physical_target_ref: Option<String>,
    #[serde(default)]
    pub target_metadata: Option<EffectWalTargetMetadata>,
    pub original: Option<EffectWalOriginal>,
    pub diagnostic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_provenance: Option<EffectExecutionProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_binding: Option<EffectReplayCommitBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_completion: Option<EffectReplayCompletion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectWalStage {
    Begin,
    BeforeImage,
    WriteApplied,
    Commit,
    RollbackComplete,
    RecoveredRollback,
    ReplayConsumed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectWalOriginal {
    pub existed: bool,
    pub content: Vec<u8>,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectWalTargetMetadata {
    pub operation_id: StableId,
    pub target_kind: EffectTargetKind,
    pub access_mode: AccessMode,
    pub content_hash: Option<String>,
    pub byte_len: u64,
    pub actor_agent_id: StableId,
    pub actor_role: ActorRole,
    pub destructive: bool,
    pub redaction_hint: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectWalRecoveryResult {
    pub status: EffectWalRecoveryStatus,
    pub recovered_transactions: Vec<String>,
    pub reasons: Vec<EffectWalRecoveryReason>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectWalRecoveryStatus {
    Noop,
    Recovered,
    RecoveryFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectWalRecoveryReason {
    NoWalFile,
    NoRecoveryNeeded,
    IncompleteTransactionRecovered,
    StoreLockFailed,
    WalReadFailed,
    WalParseFailed,
    ReservedStatePath,
    RollbackFailed,
}

/// One committed effect whose bound replay reservation has no durable
/// `replay_consumed` marker yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PendingEffectReplayCommit {
    pub tx_id: String,
    pub effect_id: StableId,
    pub provenance: EffectExecutionProvenance,
    pub replay_binding: EffectReplayCommitBinding,
}

/// Result of appending the effect-WAL half of replay completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct EffectReplayCompletionResult {
    pub wal_path: PathBuf,
    pub tx_id: String,
    pub completion: EffectReplayCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EffectReplayReconciliationError {
    InvalidRelativePath {
        field: &'static str,
        path: String,
    },
    ReservedStatePath {
        field: &'static str,
        path: String,
        reserved: ReservedStatePath,
    },
    LockScopeMismatch {
        expected: PathBuf,
        actual: PathBuf,
    },
    WalRead {
        path: PathBuf,
        source: String,
    },
    WalParse {
        path: PathBuf,
        line: usize,
        source: String,
    },
    WalRepair {
        path: PathBuf,
        source: String,
    },
    InvalidProvenance {
        tx_id: String,
        source: String,
    },
    InvalidReplayBinding {
        tx_id: String,
        reason: String,
    },
    ConflictingTransaction {
        tx_id: String,
        reason: String,
    },
    WalAppend {
        path: PathBuf,
        source: String,
    },
}

impl fmt::Display for EffectReplayReconciliationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath { field, path } => {
                write!(formatter, "invalid {field} relative path {path}")
            }
            Self::ReservedStatePath {
                field,
                path,
                reserved,
            } => write!(
                formatter,
                "{field} path {path} is reserved for EventLog TCB: {reserved:?}"
            ),
            Self::LockScopeMismatch { expected, actual } => write!(
                formatter,
                "effect lock scope mismatch: expected {}, actual {}",
                expected.display(),
                actual.display()
            ),
            Self::WalRead { path, source } => {
                write!(
                    formatter,
                    "read effect WAL {} failed: {source}",
                    path.display()
                )
            }
            Self::WalParse { path, line, source } => write!(
                formatter,
                "parse effect WAL {} line {line} failed: {source}",
                path.display()
            ),
            Self::WalRepair { path, source } => write!(
                formatter,
                "repair truncated effect WAL tail {} failed: {source}",
                path.display()
            ),
            Self::InvalidProvenance { tx_id, source } => {
                write!(
                    formatter,
                    "transaction {tx_id} has invalid provenance: {source}"
                )
            }
            Self::InvalidReplayBinding { tx_id, reason } => {
                write!(
                    formatter,
                    "transaction {tx_id} has invalid replay binding: {reason}"
                )
            }
            Self::ConflictingTransaction { tx_id, reason } => {
                write!(formatter, "transaction {tx_id} is inconsistent: {reason}")
            }
            Self::WalAppend { path, source } => write!(
                formatter,
                "append replay completion to effect WAL {} failed: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for EffectReplayReconciliationError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectTargetMetadataIndexRebuildResult {
    pub status: EffectTargetMetadataIndexRebuildStatus,
    pub rebuilt_records: usize,
    pub appended_records: usize,
    pub records: Vec<EffectTargetMetadataRecord>,
    pub reasons: Vec<EffectTargetMetadataIndexRebuildReason>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTargetMetadataIndexRebuildStatus {
    Noop,
    Rebuilt,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTargetMetadataIndexRebuildReason {
    NoWalFile,
    NoCommittedMetadataRecords,
    MetadataRebuilt,
    StoreLockFailed,
    WalReadFailed,
    WalParseFailed,
    MetadataAppendFailed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectTargetMetadataIndexQuery {
    pub logical_ref: Option<String>,
    pub effect_id: Option<StableId>,
    pub operation_id: Option<StableId>,
    pub target_kind: Option<EffectTargetKind>,
    pub latest_per_target: bool,
    pub consumer_use: EffectMetadataConsumerUse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectTargetMetadataIndexQueryResult {
    pub status: EffectTargetMetadataIndexQueryStatus,
    pub index_relative_path: String,
    pub consumer_use: EffectMetadataConsumerUse,
    pub scanned_records: usize,
    pub matched_records: usize,
    pub returned_records: usize,
    pub latest_per_target: bool,
    pub records: Vec<EffectTargetMetadataRecord>,
    pub authority_boundary: EffectMetadataAuthorityBoundary,
    pub reasons: Vec<EffectTargetMetadataIndexQueryReason>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTargetMetadataIndexQueryStatus {
    Noop,
    Queried,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTargetMetadataIndexQueryReason {
    NoIndexFile,
    QueryMatched,
    NoMatches,
    IndexReadFailed,
    IndexParseFailed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataConsumerUse {
    #[default]
    Discovery,
    Diagnostics,
    HandoffContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectMetadataAuthorityBoundary {
    pub is_workflow_authority: bool,
    pub allowed_uses: Vec<EffectMetadataConsumerUse>,
    pub forbidden_authority: Vec<EffectMetadataForbiddenAuthority>,
    pub required_authority_contracts: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataForbiddenAuthority {
    PhaseTransition,
    RouteChange,
    DecisionClose,
    CompletionClose,
    ReleaseReadiness,
    StateMutation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectMetadataContextBuildOptions {
    pub max_groups: usize,
    pub adapter_kind: Option<RuntimeKind>,
    pub adapter_trigger: EffectMetadataAdapterTrigger,
}

impl Default for EffectMetadataContextBuildOptions {
    fn default() -> Self {
        Self {
            max_groups: 20,
            adapter_kind: None,
            adapter_trigger: EffectMetadataAdapterTrigger::ManualInspection,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectMetadataContextBuildResult {
    pub status: EffectMetadataContextBuildStatus,
    pub source_status: EffectTargetMetadataIndexQueryStatus,
    pub source_consumer_use: EffectMetadataConsumerUse,
    pub total_groups: usize,
    pub returned_groups: usize,
    pub omitted_groups: usize,
    pub included_records: usize,
    pub groups: Vec<EffectMetadataContextGroup>,
    pub adapter_presentation: EffectMetadataAdapterPresentation,
    pub authority_boundary: EffectMetadataAuthorityBoundary,
    pub reasons: Vec<EffectMetadataContextBuildReason>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataContextBuildStatus {
    Empty,
    Built,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataContextBuildReason {
    NoQueryRecords,
    ContextBuilt,
    GroupsOmittedByLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectMetadataContextGroup {
    pub target_kind: EffectTargetKind,
    pub logical_ref: String,
    pub record_count: usize,
    pub latest_physical_ref: String,
    pub latest_effect_id: StableId,
    pub latest_operation_id: StableId,
    pub latest_recorded_at: Option<String>,
    pub latest_access_mode: AccessMode,
    pub latest_content_hash: Option<String>,
    pub latest_byte_len: u64,
    pub latest_actor_agent_id: StableId,
    pub latest_actor_role: ActorRole,
    pub destructive: bool,
    pub redaction_hint: StableId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataAdapterTrigger {
    EvidenceDiscovery,
    Diagnostics,
    HandoffPreparation,
    #[default]
    ManualInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectMetadataAdapterPresentation {
    pub adapter_kind: Option<RuntimeKind>,
    pub trigger: EffectMetadataAdapterTrigger,
    pub automatic_invocation_allowed: bool,
    pub presentation_mode: EffectMetadataPresentationMode,
    pub may_create_workflow_authority: bool,
    pub required_output_treatment: Vec<EffectMetadataOutputTreatment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataPresentationMode {
    AdvisoryContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMetadataOutputTreatment {
    PreserveAuthorityBoundary,
    DoNotSummarizeAsNextAction,
    DoNotMutateStateFromContext,
    KeepRawContentOmitted,
}

/// Narrow retained view of the exact authoritative member bound to one
/// designated effect lock. The private fields prevent construction without a
/// live lock capability, and every open rechecks the lock-to-member pairing.
#[derive(Debug, Clone, Copy)]
pub struct RetainedStateRoot<'a> {
    inner: &'a retained_dir::RetainedDirectory,
    lock_relative_path: &'a Path,
}

impl RetainedStateRoot<'_> {
    #[must_use]
    pub fn display_path(&self) -> &Path {
        self.inner.display_path()
    }

    fn validate_member_path(&self, path: &Path) -> io::Result<()> {
        let permitted = matches!(
            (self.lock_relative_path.to_str(), path.to_str()),
            (Some("locks/memory.log.lock"), Some("memory/events.ndjson"))
                | (
                    Some("locks/research.sources.lock"),
                    Some("research/sources.ndjson")
                )
                | (
                    Some("locks/governance.conflicts.lock"),
                    Some("governance/conflicts.ndjson")
                )
                | (
                    Some("locks/workflow-governance.lock"),
                    Some("wal/workflow-governance.ndjson")
                )
        );
        if permitted {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained state-root access is limited to the authority member bound to this lock",
            ))
        }
    }

    fn validate_writable_eventlog_path(&self, path: &Path) -> io::Result<()> {
        let permitted = matches!(
            (self.lock_relative_path.to_str(), path.to_str()),
            (Some("locks/memory.log.lock"), Some("memory/events.ndjson"))
                | (
                    Some("locks/research.sources.lock"),
                    Some("research/sources.ndjson")
                )
                | (
                    Some("locks/governance.conflicts.lock"),
                    Some("governance/conflicts.ndjson")
                )
        );
        if permitted {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained writable state-root access is limited to the EventLog member bound to this lock",
            ))
        }
    }

    /// Open the exact authority member bound to this retained lock for reading.
    ///
    /// # Errors
    ///
    /// Returns an I/O error for a lock/member mismatch, unsafe traversal,
    /// missing entry, or open failure.
    pub fn open_read(&self, path: &Path) -> io::Result<File> {
        self.validate_member_path(path)?;
        self.inner.open_read(path)
    }

    /// Open or create the exact `EventLog` member bound to this retained lock.
    ///
    /// # Errors
    ///
    /// Returns an I/O error for a lock/member mismatch, unsafe traversal,
    /// unsupported entry type, or open failure.
    pub fn open_read_write_create(&self, path: &Path) -> io::Result<File> {
        self.validate_writable_eventlog_path(path)?;
        self.inner.open_read_write_create(path)
    }
}

/// Opaque Store-owned authority for one retained effect root.
///
/// Construction acquires the producer boundary before exposing the capability.
/// Subsequent effect-lock opens are descriptor-relative to that exact retained
/// root and reuse the same unforgeable boundary lease.
#[derive(Debug)]
pub struct RetainedEffectStoreRoot {
    root: retained_dir::RetainedDirectory,
    boundary: producer_quiescence::BoundaryLease,
}

impl RetainedEffectStoreRoot {
    /// Acquire effect authority for one exact root and retain its directory handle.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError`] if the producer boundary cannot be
    /// acquired or its retained root handle cannot be duplicated.
    pub fn acquire(root: impl AsRef<Path>) -> Result<Self, EffectStoreLockError> {
        let root = root.as_ref();
        let guard = producer_quiescence::admit_effect_producer(root, false)
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        let boundary = producer_quiescence::BoundaryLease::from_boundary(&guard, root)
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        let retained_root = boundary
            .retained_root()
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        Ok(Self {
            root: retained_root,
            boundary,
        })
    }

    /// Acquire an effect lock beneath this exact retained root.
    ///
    /// The lock path is state-root-relative. Legacy `.forge-method` sidecar
    /// paths must use [`acquire_effect_store_lock`] so their boundary root can be
    /// selected before authority is acquired.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError`] for an invalid path, retained-root
    /// mismatch, open failure, or lock contention.
    pub fn acquire_effect_store_lock(
        &self,
        lock_relative_path: &str,
    ) -> Result<EffectStoreLock, EffectStoreLockError> {
        if effect_uses_legacy_sidecar(lock_relative_path) {
            return Err(EffectStoreLockError::InvalidRelativePath {
                path: lock_relative_path.to_owned(),
            });
        }
        self.validate()?;
        let lock = acquire_effect_store_lock_inner(
            self.root.display_path(),
            lock_relative_path,
            false,
            false,
            self.boundary.clone(),
            false,
        )?;
        let expected = self
            .root
            .identity()
            .map_err(|source| EffectStoreLockError::OpenFile {
                path: self.root.display_path().to_path_buf(),
                source: format!("validate retained effect-root identity: {source}"),
            })?;
        let actual =
            lock.state_root
                .identity()
                .map_err(|source| EffectStoreLockError::OpenFile {
                    path: self.root.display_path().to_path_buf(),
                    source: format!("validate acquired effect-lock root identity: {source}"),
                })?;
        if actual != expected {
            return Err(EffectStoreLockError::OpenFile {
                path: self.root.display_path().to_path_buf(),
                source: "acquired effect lock is not bound to the retained root".to_owned(),
            });
        }
        Ok(lock)
    }

    fn validate(&self) -> Result<(), EffectStoreLockError> {
        self.boundary
            .require_effect_authority()
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        let boundary_root = self
            .boundary
            .retained_root()
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        let retained_identity =
            self.root
                .identity()
                .map_err(|source| EffectStoreLockError::OpenFile {
                    path: self.root.display_path().to_path_buf(),
                    source: format!("validate retained effect-root identity: {source}"),
                })?;
        let boundary_identity =
            boundary_root
                .identity()
                .map_err(|source| EffectStoreLockError::OpenFile {
                    path: self.root.display_path().to_path_buf(),
                    source: format!("validate producer-boundary root identity: {source}"),
                })?;
        if retained_identity != boundary_identity {
            return Err(EffectStoreLockError::OpenFile {
                path: self.root.display_path().to_path_buf(),
                source: "retained effect root no longer matches its producer boundary".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EffectStoreLock {
    // The boundary retains the state-root inode exclusively before `file`; this
    // serializes cooperating effects even if the entire child lock namespace is
    // removed and recreated.
    file: File,
    path: PathBuf,
    lock_relative_path: PathBuf,
    state_lock_relative_path: PathBuf,
    root: retained_dir::RetainedDirectory,
    state_root: retained_dir::RetainedDirectory,
    lock_identity: retained_dir::RetainedFileIdentity,
    boundary: producer_quiescence::BoundaryLease,
}

/// Sealed descriptor-relative I/O beneath the exact directory containing an
/// already-held effect lock.
///
/// The capability retains the state root, lock-parent directory, and lock
/// identity. Its path is diagnostic only. Reads, create-new writes, recovery,
/// and replacement never resolve through that ambient path.
#[derive(Debug)]
pub struct RetainedEffectStoreIo<'lock> {
    lock: &'lock EffectStoreLock,
    directory: retained_dir::RetainedDirectory,
    directory_relative_path: PathBuf,
    directory_identity: retained_dir::RetainedFileIdentity,
    state_root_identity: retained_dir::RetainedFileIdentity,
}

/// Opaque witness for one exact retained Store authority leaf.
///
/// The witness owns the retained leaf handle plus the exact state-root and
/// parent-directory identities under which the leaf was observed. Its path is
/// diagnostic only. Revalidation is descriptor-relative and verifies the root,
/// parent, leaf, exact bytes, digest, producer boundary, and lock binding.
pub struct RetainedEffectStoreLeafWitness<'lock> {
    lock: &'lock EffectStoreLock,
    parent: retained_dir::RetainedDirectory,
    parent_relative_path: PathBuf,
    state_root_identity: retained_dir::RetainedFileIdentity,
    parent_identity: retained_dir::RetainedFileIdentity,
    relative_path: PathBuf,
    file: File,
    leaf_identity: retained_dir::RetainedFileIdentity,
    bytes: Vec<u8>,
    digest: String,
    maximum: u64,
}

/// Opaque proof that one exact retained Store authority leaf was absent.
///
/// The proof retains the same root, parent, producer lock, and leaf binding as a
/// present witness. It is non-constructible outside Store and revalidates absence
/// descriptor-relatively, so callers cannot turn an unrelated `None` or pathname
/// observation into replacement authority.
pub struct RetainedEffectStoreLeafAbsenceWitness<'lock> {
    lock: &'lock EffectStoreLock,
    parent: retained_dir::RetainedDirectory,
    parent_relative_path: PathBuf,
    state_root_identity: retained_dir::RetainedFileIdentity,
    parent_identity: retained_dir::RetainedFileIdentity,
    relative_path: PathBuf,
}

/// Exact expected state accepted by public crash replacement.
///
/// Both variants are minted by Store for one exact lock/root/parent/leaf. A
/// present state retains the exact file handle and bytes; an absent state retains
/// descriptor-relative absence authority for that same leaf.
pub enum RetainedEffectStoreExpectedLeaf<'lock> {
    Present(RetainedEffectStoreLeafWitness<'lock>),
    Absent(RetainedEffectStoreLeafAbsenceWitness<'lock>),
}

/// Descriptor-bound replay authority derived from an already-held effect lock.
/// Its display path is diagnostic only; all replay I/O starts at `state_root`.
pub(crate) struct RetainedEffectReplayScope {
    pub(crate) state_root: retained_dir::RetainedDirectory,
    pub(crate) state_root_display: PathBuf,
    pub(crate) boundary: producer_quiescence::BoundaryLease,
}

impl EffectStoreLock {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Transfer one exact retained project tree into durable Store-owned anchors.
    ///
    /// Every anchor is created descriptor-relatively beneath this lock's exact
    /// retained state root and directly from the project's already-retained file
    /// handles. The persisted binding contains random private anchor nonces,
    /// content digests, lengths, and project/snapshot digests, but no reusable
    /// device/inode or volume/index identity. The returned live capability keeps
    /// both sides of every exact-file binding open through selector publication.
    ///
    /// # Errors
    ///
    /// Fails closed if the producer boundary, effect lock, retained project,
    /// anchor directory, exact-handle hard-link primitive, or resulting anchor set
    /// cannot be validated. Platforms without exact handle-bound anchor creation
    /// return an unsupported I/O error rather than persisting weaker authority.
    pub fn retain_project_tree_anchors(
        &self,
        project_tree: &retained_project_tree::RetainedProjectTree,
        anchor_directory: impl AsRef<Path>,
    ) -> Result<
        retained_project_tree::RetainedProjectLifetimeAnchors,
        retained_project_tree::RetainedProjectTreeError,
    > {
        self.validate_project_anchor_store_authority("retain project-tree anchors")?;
        let anchors =
            project_tree.retain_completion_anchors(&self.state_root, anchor_directory.as_ref())?;
        self.validate_project_anchor_store_authority("validate retained project-tree anchors")?;
        anchors.revalidate()?;
        Ok(anchors)
    }

    /// Reopen a durable project-tree anchor binding under a fresh Store lock and
    /// cross-bind it to the caller's newly retained project handles.
    ///
    /// Identical bytes, reminted project capability nonces, or reused platform
    /// object identifiers are insufficient: every current project file must be
    /// the exact object kept alive by the persisted private Store anchor.
    ///
    /// # Errors
    ///
    /// Fails closed if the lock/root authority changed, the binding is malformed
    /// or belongs to another path/snapshot, an anchor is missing or modified, or
    /// any current project file is not the exact anchored object.
    pub fn open_project_tree_anchors(
        &self,
        project_tree: &retained_project_tree::RetainedProjectTree,
        binding: &retained_project_tree::RetainedProjectAnchorBinding,
    ) -> Result<
        retained_project_tree::RetainedProjectLifetimeAnchors,
        retained_project_tree::RetainedProjectTreeError,
    > {
        self.validate_project_anchor_store_authority("open project-tree anchors")?;
        let anchors = project_tree.open_completion_anchors(&self.state_root, binding)?;
        self.validate_project_anchor_store_authority("validate opened project-tree anchors")?;
        anchors.revalidate()?;
        Ok(anchors)
    }

    fn validate_project_anchor_store_authority(
        &self,
        action: &str,
    ) -> Result<(), retained_project_tree::RetainedProjectTreeError> {
        self.boundary
            .require_effect_authority()
            .and_then(|()| self.boundary.validate_root(self.state_root.display_path()))
            .map_err(
                |error| retained_project_tree::RetainedProjectTreeError::Io {
                    path: self.state_root.display_path().to_path_buf(),
                    reason: format!("{action}: producer boundary validation failed: {error}"),
                },
            )?;
        self.validate_retained_lock_file().map_err(|error| {
            retained_project_tree::RetainedProjectTreeError::Io {
                path: self.path.clone(),
                reason: format!("{action}: retained Store lock validation failed: {error}"),
            }
        })
    }

    /// Derive sealed I/O authority beneath this exact effect lock's parent.
    ///
    /// The returned capability rejects the lock leaf itself. Every operation
    /// revalidates the producer-bound ambient root name, retained state-root
    /// identity, retained lock-parent binding, and exact lock-file identity.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError`] if any retained binding changed or the
    /// lock parent cannot be retained descriptor-relatively.
    pub fn retained_store_io(&self) -> Result<RetainedEffectStoreIo<'_>, EffectStoreLockError> {
        self.boundary
            .require_effect_authority()
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        self.boundary
            .validate_root(self.state_root.display_path())
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        self.validate_retained_lock_file()?;
        let directory_relative_path = self
            .state_lock_relative_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let directory = if directory_relative_path.as_os_str().is_empty() {
            self.state_root.try_clone()
        } else {
            self.state_root.open_directory(&directory_relative_path)
        }
        .map_err(|source| EffectStoreLockError::OpenFile {
            path: self
                .state_root
                .display_path()
                .join(&directory_relative_path),
            source: format!("retain effect-lock parent directory: {source}"),
        })?;
        let directory_identity =
            directory
                .identity()
                .map_err(|source| EffectStoreLockError::OpenFile {
                    path: directory.display_path().to_path_buf(),
                    source: format!("retain effect-lock parent identity: {source}"),
                })?;
        let state_root_identity =
            self.state_root
                .identity()
                .map_err(|source| EffectStoreLockError::OpenFile {
                    path: self.state_root.display_path().to_path_buf(),
                    source: format!("retain effect state-root identity: {source}"),
                })?;
        let capability = RetainedEffectStoreIo {
            lock: self,
            directory,
            directory_relative_path,
            directory_identity,
            state_root_identity,
        };
        capability
            .validate()
            .map_err(|source| EffectStoreLockError::OpenFile {
                path: capability.directory.display_path().to_path_buf(),
                source: format!("validate retained effect-store I/O capability: {source}"),
            })?;
        Ok(capability)
    }

    /// Exact state-root directory retained by this lock's producer boundary.
    /// Child access is descriptor-relative and remains capability-gated by the
    /// non-constructible lock guard.
    #[doc(hidden)]
    #[must_use]
    pub fn retained_state_root(&self) -> RetainedStateRoot<'_> {
        RetainedStateRoot {
            inner: &self.state_root,
            lock_relative_path: &self.state_lock_relative_path,
        }
    }

    /// Diagnostic pathname bound to the retained state-root handle.
    #[doc(hidden)]
    #[must_use]
    pub fn retained_state_root_path(&self) -> &Path {
        self.state_root.display_path()
    }

    /// Verify that the retained lock still names the exact locked file.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError::OpenFile`] if the lock entry disappeared,
    /// was substituted, or no longer has the retained file identity.
    pub fn validate_retained_lock_file(&self) -> Result<(), EffectStoreLockError> {
        let current_identity = self
            .state_root
            .open_leaf_read(
                &self.state_lock_relative_path,
                retained_dir::RetainedLeafPolicy::Authority,
            )
            .and_then(|file| retained_dir::RetainedDirectory::identity_of(&file))
            .map_err(|source| EffectStoreLockError::OpenFile {
                path: self.path.clone(),
                source: format!("validate retained effect lock identity: {source}"),
            })?;
        if current_identity != self.lock_identity {
            return Err(EffectStoreLockError::OpenFile {
                path: self.path.clone(),
                source: "retained effect lock identity changed".to_owned(),
            });
        }
        Ok(())
    }

    /// Derive opaque crash-replacement authority for one exact retained target.
    ///
    /// The root, parent, leaf, and producer lock are privately bound beneath the
    /// exact state-root handle already retained by this lock. Callers receive no
    /// generic filesystem mutation surface and cannot retarget the capability.
    ///
    /// # Errors
    ///
    /// Returns [`EffectStoreLockError`] if the target is not normalized, names a
    /// reserved `EventLog` path or the lock itself, the held lock changed identity,
    /// or the retained parent cannot be opened.
    pub(crate) fn retained_crash_replace_target(
        &self,
        target_relative: &Path,
    ) -> Result<retained_crash_replace::RetainedCrashReplaceTarget<'_>, EffectStoreLockError> {
        if target_relative.as_os_str().is_empty()
            || target_relative.is_absolute()
            || target_relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(EffectStoreLockError::InvalidRelativePath {
                path: target_relative.display().to_string(),
            });
        }
        let target_text =
            target_relative
                .to_str()
                .ok_or_else(|| EffectStoreLockError::InvalidRelativePath {
                    path: target_relative.display().to_string(),
                })?;
        match reject_reserved_state_mutation(target_text) {
            Ok(_) => {}
            Err(ReservedStateMutationError::Invalid) => {
                return Err(EffectStoreLockError::InvalidRelativePath {
                    path: target_text.to_owned(),
                });
            }
            Err(ReservedStateMutationError::Reserved(reserved)) => {
                return Err(EffectStoreLockError::ReservedStatePath {
                    path: target_text.to_owned(),
                    reserved,
                });
            }
        }
        let target_name = target_relative
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| EffectStoreLockError::InvalidRelativePath {
                path: target_text.to_owned(),
            })?;
        if ["forge-next", "forge-previous", "forge-transaction"]
            .iter()
            .any(|suffix| target_name.ends_with(suffix))
            || target_relative == self.state_lock_relative_path
        {
            return Err(EffectStoreLockError::InvalidRelativePath {
                path: target_text.to_owned(),
            });
        }
        let target_parent_relative = target_relative.parent().unwrap_or_else(|| Path::new(""));
        self.boundary
            .require_effect_authority()
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
        self.validate_retained_lock_file()?;
        let directory = if target_parent_relative.as_os_str().is_empty() {
            self.state_root.try_clone()
        } else {
            self.state_root
                .create_dir_all(target_parent_relative)
                .and_then(|()| self.state_root.open_directory(target_parent_relative))
        }
        .map_err(|source| EffectStoreLockError::OpenFile {
            path: self.state_root.display_path().join(target_parent_relative),
            source: format!("open retained crash-replacement parent: {source}"),
        })?;
        let authority = retained_crash_replace::RetainedCrashReplaceTarget::new(
            self,
            directory,
            target_relative.to_path_buf(),
        )
        .map_err(|source| EffectStoreLockError::OpenFile {
            path: self.state_root.display_path().join(target_relative),
            source: format!("bind retained crash-replacement target: {source}"),
        })?;
        self.validate_retained_lock_file()?;
        Ok(authority)
    }

    pub(crate) fn replay_scope(
        &self,
        expected_state_lock_relative: &str,
    ) -> Result<RetainedEffectReplayScope, String> {
        let relative = Path::new(expected_state_lock_relative);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(
                "expected effect lock must be a normalized non-empty state-root-relative path"
                    .to_owned(),
            );
        }
        if relative == Path::new(replay_wal::REPLAY_WAL_LOCK_RELATIVE_PATH) {
            return Err(
                "effect lock aliases replay lock; required order is effect lock then replay lock"
                    .to_owned(),
            );
        }
        if relative != self.state_lock_relative_path {
            return Err(
                "expected effect lock does not match held state-root-relative lock".to_owned(),
            );
        }
        let state_identity = self
            .state_root
            .identity()
            .map_err(|error| error.to_string())?;
        if self.lock_relative_path == self.state_lock_relative_path {
            if self.root.identity().map_err(|error| error.to_string())? != state_identity {
                return Err("direct effect root no longer matches retained state root".to_owned());
            }
        } else {
            let legacy = self
                .root
                .open_directory(Path::new(".forge-method"))
                .map_err(|error| error.to_string())?;
            if legacy.identity().map_err(|error| error.to_string())? != state_identity {
                return Err(
                    "legacy effect sidecar no longer matches retained state root".to_owned(),
                );
            }
        }
        let identity = self
            .state_root
            .open_leaf_read(
                &self.state_lock_relative_path,
                retained_dir::RetainedLeafPolicy::Authority,
            )
            .and_then(|file| retained_dir::RetainedDirectory::identity_of(&file))
            .map_err(|error| error.to_string())?;
        if identity != self.lock_identity {
            return Err("held effect lock identity changed".to_owned());
        }
        Ok(RetainedEffectReplayScope {
            state_root: self
                .state_root
                .try_clone()
                .map_err(|error| error.to_string())?,
            state_root_display: self.state_root.display_path().to_path_buf(),
            boundary: self.boundary.clone(),
        })
    }
}

impl fmt::Debug for RetainedEffectStoreLeafWitness<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedEffectStoreLeafWitness")
            .field("relative_path", &self.relative_path)
            .field("byte_length", &self.bytes.len())
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RetainedEffectStoreLeafAbsenceWitness<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedEffectStoreLeafAbsenceWitness")
            .field("relative_path", &self.relative_path)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for RetainedEffectStoreExpectedLeaf<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Present(witness) => formatter
                .debug_tuple("RetainedEffectStoreExpectedLeaf::Present")
                .field(witness)
                .finish(),
            Self::Absent(witness) => formatter
                .debug_tuple("RetainedEffectStoreExpectedLeaf::Absent")
                .field(witness)
                .finish(),
        }
    }
}

impl<'lock> RetainedEffectStoreLeafWitness<'lock> {
    /// Exact bytes read from the retained leaf handle.
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// SHA-256 content digest of [`Self::raw_bytes`].
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Revalidate the retained root, parent, leaf, bytes, digest, lock, and
    /// producer boundary without resolving through the diagnostic pathname.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if any retained identity or exact byte binding
    /// changed.
    pub fn revalidate(&mut self) -> io::Result<()> {
        self.validate_namespace()?;
        self.parent.verify_retained_authority_binding(
            &self.relative_path,
            &self.file,
            &self.leaf_identity,
        )?;
        let actual = read_retained_effect_leaf(&mut self.file, self.maximum)?;
        if actual != self.bytes || sha256_content_hash(&actual) != self.digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained effect-store leaf bytes or digest changed",
            ));
        }
        self.parent.verify_retained_authority_binding(
            &self.relative_path,
            &self.file,
            &self.leaf_identity,
        )?;
        self.validate_namespace()
    }

    fn validate_namespace(&self) -> io::Result<()> {
        self.lock
            .boundary
            .require_effect_authority()
            .and_then(|()| {
                self.lock
                    .boundary
                    .validate_root(self.lock.state_root.display_path())
            })
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        self.lock
            .validate_retained_lock_file()
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        if self.lock.state_root.identity()? != self.state_root_identity
            || self.parent.identity()? != self.parent_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-store leaf root or parent identity changed",
            ));
        }
        let current_parent = if self.parent_relative_path.as_os_str().is_empty() {
            self.lock.state_root.try_clone()
        } else {
            self.lock
                .state_root
                .open_directory(&self.parent_relative_path)
        }?;
        if current_parent.identity()? != self.parent_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-store leaf parent is no longer bound beneath the retained root",
            ));
        }
        Ok(())
    }

    fn is_bound_to(&self, io: &RetainedEffectStoreIo<'lock>, relative: &Path) -> bool {
        std::ptr::eq(self.lock, io.lock)
            && self.parent_relative_path == io.directory_relative_path
            && self.state_root_identity == io.state_root_identity
            && self.parent_identity == io.directory_identity
            && self.relative_path == relative
    }
}

impl<'lock> RetainedEffectStoreLeafAbsenceWitness<'lock> {
    /// Revalidate the exact root, parent, lock, leaf, and continued absence.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if any retained namespace binding changed or the
    /// exact authority leaf now exists.
    pub fn revalidate(&mut self) -> io::Result<()> {
        self.validate_namespace()?;
        match self.parent.open_leaf_read(
            &self.relative_path,
            retained_dir::RetainedLeafPolicy::Authority,
        ) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => self.validate_namespace(),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "retained effect-store absence authority leaf now exists",
            )),
            Err(error) => Err(error),
        }
    }

    fn validate_namespace(&self) -> io::Result<()> {
        self.lock
            .boundary
            .require_effect_authority()
            .and_then(|()| {
                self.lock
                    .boundary
                    .validate_root(self.lock.state_root.display_path())
            })
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        self.lock
            .validate_retained_lock_file()
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        if self.lock.state_root.identity()? != self.state_root_identity
            || self.parent.identity()? != self.parent_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-store absence root or parent identity changed",
            ));
        }
        let current_parent = if self.parent_relative_path.as_os_str().is_empty() {
            self.lock.state_root.try_clone()
        } else {
            self.lock
                .state_root
                .open_directory(&self.parent_relative_path)
        }?;
        if current_parent.identity()? != self.parent_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-store absence parent is no longer bound beneath the retained root",
            ));
        }
        Ok(())
    }

    fn is_bound_to(&self, io: &RetainedEffectStoreIo<'lock>, relative: &Path) -> bool {
        std::ptr::eq(self.lock, io.lock)
            && self.parent_relative_path == io.directory_relative_path
            && self.state_root_identity == io.state_root_identity
            && self.parent_identity == io.directory_identity
            && self.relative_path == relative
    }
}

impl<'lock> RetainedEffectStoreExpectedLeaf<'lock> {
    /// Digest of the exact present leaf, or `None` for exact absence.
    #[must_use]
    pub fn digest(&self) -> Option<&str> {
        match self {
            Self::Present(witness) => Some(witness.digest()),
            Self::Absent(_) => None,
        }
    }

    /// Exact present bytes, or `None` for exact absence.
    #[must_use]
    pub fn raw_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Present(witness) => Some(witness.raw_bytes()),
            Self::Absent(_) => None,
        }
    }

    /// Revalidate the exact present handle or exact descriptor-relative absence.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if any retained lock/root/parent/leaf binding changed.
    pub fn revalidate(&mut self) -> io::Result<()> {
        match self {
            Self::Present(witness) => witness.revalidate(),
            Self::Absent(witness) => witness.revalidate(),
        }
    }

    fn is_bound_to(&self, io: &RetainedEffectStoreIo<'lock>, relative: &Path) -> bool {
        match self {
            Self::Present(witness) => witness.is_bound_to(io, relative),
            Self::Absent(witness) => witness.is_bound_to(io, relative),
        }
    }
}

fn canonical_retained_path_bytes(path: &Path) -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        Ok(path.as_os_str().as_bytes().to_vec())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt as _;
        Ok(path
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_be_bytes)
            .collect())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "canonical retained path encoding is unsupported on this platform",
        ))
    }
}

fn read_retained_effect_leaf(file: &mut File, maximum: u64) -> io::Result<Vec<u8>> {
    let before = file.metadata()?;
    if before.len() > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained effect-store file exceeds byte limit",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    Read::by_ref(file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "retained effect-store file exceeds byte limit",
        ));
    }
    let after = file.metadata()?;
    if after.len() != before.len() || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained effect-store file changed while read",
        ));
    }
    Ok(bytes)
}

impl<'lock> RetainedEffectStoreIo<'lock> {
    /// Diagnostic path of the retained lock-parent directory.
    #[must_use]
    pub fn display_path(&self) -> &Path {
        self.directory.display_path()
    }

    /// Retain one exact descendant directory beneath this capability.
    ///
    /// Missing components are created descriptor-relatively and the containing
    /// retained directory is synchronized before the child capability is
    /// returned. The child independently revalidates its exact namespace
    /// binding on every operation.
    ///
    /// # Errors
    ///
    /// Returns an I/O error for an invalid path, unsafe component, changed
    /// retained binding, directory creation failure, or synchronization failure.
    pub fn retain_subdirectory(&self, relative: &Path) -> io::Result<RetainedEffectStoreIo<'lock>> {
        self.validate_relative(relative)?;
        if relative.components().count() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained subdirectory must be one direct normal child",
            ));
        }
        self.validate()?;
        self.directory.create_dir_all(relative)?;
        self.directory.sync_root()?;
        let directory = self.directory.open_directory(relative)?;
        let directory_identity = directory.identity()?;
        let capability = RetainedEffectStoreIo {
            lock: self.lock,
            directory,
            directory_relative_path: self.directory_relative_path.join(relative),
            directory_identity,
            state_root_identity: self.state_root_identity.clone(),
        };
        capability.validate()?;
        self.validate()?;
        Ok(capability)
    }

    /// Revalidate the exact root, parent, and lock bindings retained by this
    /// capability.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the ambient root was renamed or substituted, the
    /// retained handles changed identity, the lock parent was rebound, or the
    /// locked leaf changed identity.
    pub fn validate(&self) -> io::Result<()> {
        self.lock
            .boundary
            .require_effect_authority()
            .and_then(|()| {
                self.lock
                    .boundary
                    .validate_root(self.lock.state_root.display_path())
            })
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        self.lock
            .validate_retained_lock_file()
            .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error.to_string()))?;
        if self.lock.state_root.identity()? != self.state_root_identity
            || self.directory.identity()? != self.directory_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-store I/O binding changed",
            ));
        }
        let current = if self.directory_relative_path.as_os_str().is_empty() {
            self.lock.state_root.try_clone()
        } else {
            self.lock
                .state_root
                .open_directory(&self.directory_relative_path)
        }?;
        if current.identity()? != self.directory_identity {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-lock parent is no longer bound beneath the retained root",
            ));
        }
        Ok(())
    }

    /// Opaque digest binding this exact retained state root, producer lock, and
    /// descriptor-relative authority directory. The digest is persisted only as
    /// context inside higher-level immutable authority records; it grants no I/O
    /// authority by itself.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if any retained root, directory, or lock binding has
    /// changed while the digest is derived.
    #[doc(hidden)]
    pub fn authority_binding_digest(&self) -> io::Result<String> {
        self.validate()?;
        let state_root_identity = self.state_root_identity.canonical_digest()?;
        let directory_identity = self.directory_identity.canonical_digest()?;
        let lock_identity = self.lock.lock_identity.canonical_digest()?;
        let directory_relative = canonical_retained_path_bytes(&self.directory_relative_path)?;
        let lock_relative = canonical_retained_path_bytes(&self.lock.state_lock_relative_path)?;
        let mut descriptor = b"forge-method:retained-effect-store-authority:v1\0".to_vec();
        for field in [
            state_root_identity.as_bytes(),
            directory_identity.as_bytes(),
            lock_identity.as_bytes(),
            directory_relative.as_slice(),
            lock_relative.as_slice(),
        ] {
            descriptor
                .extend_from_slice(&u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
            descriptor.extend_from_slice(field);
        }
        let digest = sha256_content_hash(&descriptor);
        self.validate()?;
        Ok(digest)
    }

    /// Mint one Store-owned random operation identity while this exact retained
    /// root and producer lock remain valid.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if randomness fails or the retained authority changes.
    #[doc(hidden)]
    pub fn mint_operation_nonce(&self) -> io::Result<String> {
        self.validate()?;
        let mut nonce = [0_u8; 32];
        getrandom::fill(&mut nonce).map_err(|error| {
            io::Error::other(format!("operation nonce generation failed: {error}"))
        })?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(nonce.len() * 2);
        for byte in nonce {
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        self.validate()?;
        Ok(encoded)
    }

    fn validate_relative(&self, relative: &Path) -> io::Result<()> {
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained effect-store I/O path is not a normalized relative child",
            ));
        }
        let state_relative = self.directory_relative_path.join(relative);
        let state_text = state_relative.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained effect-store I/O path is not UTF-8",
            )
        })?;
        if state_relative == self.lock.state_lock_relative_path
            || reject_reserved_state_mutation(state_text).is_err()
            || state_relative.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                ["forge-next", "forge-previous", "forge-transaction"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "retained effect-store I/O path names protected Store authority",
            ));
        }
        Ok(())
    }

    fn state_relative(&self, relative: &Path) -> io::Result<PathBuf> {
        self.validate_relative(relative)?;
        Ok(self.directory_relative_path.join(relative))
    }

    fn validate_leaf_relative(&self, relative: &Path) -> io::Result<()> {
        self.validate_relative(relative)?;
        if relative.components().count() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "retained effect-store leaf witness requires one direct normal child",
            ));
        }
        Ok(())
    }

    fn witness_from_retained_file(
        &self,
        relative: &Path,
        mut file: File,
        leaf_identity: retained_dir::RetainedFileIdentity,
        maximum: u64,
        expected_bytes: Option<&[u8]>,
    ) -> io::Result<RetainedEffectStoreLeafWitness<'lock>> {
        self.validate_leaf_relative(relative)?;
        self.validate()?;
        self.directory
            .verify_retained_authority_binding(relative, &file, &leaf_identity)?;
        let bytes = read_retained_effect_leaf(&mut file, maximum)?;
        if expected_bytes.is_some_and(|expected| expected != bytes.as_slice()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained effect-store leaf differs from committed exact bytes",
            ));
        }
        self.directory
            .verify_retained_authority_binding(relative, &file, &leaf_identity)?;
        let mut witness = RetainedEffectStoreLeafWitness {
            lock: self.lock,
            parent: self.directory.try_clone()?,
            parent_relative_path: self.directory_relative_path.clone(),
            state_root_identity: self.state_root_identity.clone(),
            parent_identity: self.directory_identity.clone(),
            relative_path: relative.to_path_buf(),
            file,
            leaf_identity,
            digest: sha256_content_hash(&bytes),
            bytes,
            maximum,
        };
        witness.revalidate()?;
        self.validate()?;
        Ok(witness)
    }

    fn retain_optional_leaf(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> io::Result<Option<RetainedEffectStoreLeafWitness<'lock>>> {
        self.validate_leaf_relative(relative)?;
        self.validate()?;
        let file = match self
            .directory
            .open_leaf_read(relative, retained_dir::RetainedLeafPolicy::Authority)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.validate()?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let identity = retained_dir::RetainedDirectory::identity_of(&file)?;
        self.witness_from_retained_file(relative, file, identity, maximum, None)
            .map(Some)
    }

    /// Read and retain one optional exact regular file through the retained lock
    /// parent, enforcing a byte limit. Unix may retain Store-owned publication
    /// aliases as discoverable cleanup debt.
    ///
    /// The successful result keeps the root, parent, leaf, bytes, and digest
    /// bound until its witness is dropped.
    ///
    /// # Errors
    ///
    /// Returns an I/O error for an invalid path, unsafe entry, size overflow,
    /// changed retained binding, or read failure.
    pub fn read_optional_bounded(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> io::Result<Option<RetainedEffectStoreLeafWitness<'lock>>> {
        self.retain_optional_leaf(relative, maximum)
    }

    /// Create and durably sync one new regular file beneath the retained lock
    /// parent, returning the exact created leaf witness.
    ///
    /// The operation is create-new only. It never truncates or replaces an
    /// existing leaf. If a durable write cannot complete, Store does not perform
    /// a pathname-only cleanup; any surviving partial leaf remains discoverable
    /// at its exact target name as bounded recovery debt.
    ///
    /// # Errors
    ///
    /// Returns an I/O error for an invalid path, oversized content, an existing
    /// leaf, changed retained binding, or durable write failure.
    pub fn write_new_file_synced(
        &self,
        relative: &Path,
        content: &[u8],
        maximum: u64,
    ) -> io::Result<RetainedEffectStoreLeafWitness<'lock>> {
        self.validate_leaf_relative(relative)?;
        if u64::try_from(content.len()).unwrap_or(u64::MAX) > maximum {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "retained effect-store content exceeds byte limit",
            ));
        }
        self.validate()?;
        let mut writer = self.directory.open_leaf_write_new_authority(relative)?;
        let identity = retained_dir::RetainedDirectory::identity_of(&writer)?;
        if let Err(error) = writer
            .write_all(content)
            .and_then(|()| writer.sync_all())
            .and_then(|()| {
                self.directory
                    .verify_retained_authority_binding(relative, &writer, &identity)
            })
            .and_then(|()| self.directory.sync_root())
        {
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "{error}; incomplete retained create-new leaf remains discoverable as recovery debt at {}",
                    self.directory.display_path().join(relative).display()
                ),
            ));
        }
        let reader = self
            .directory
            .open_leaf_read(relative, retained_dir::RetainedLeafPolicy::Authority)?;
        if retained_dir::RetainedDirectory::identity_of(&reader)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "retained create-new leaf was substituted before its witness was retained",
            ));
        }
        let witness =
            self.witness_from_retained_file(relative, reader, identity, maximum, Some(content))?;
        self.validate()?;
        Ok(witness)
    }

    /// Reconcile one crash-replacement target into Store-owned one-shot
    /// authority.
    ///
    /// The returned session owns this capability's exact lock, root, parent, and
    /// leaf binding together with marker finalization's exact present handle or
    /// Store-minted absence authority. It may cross higher-level work and then be
    /// consumed exactly once for replacement or an exact read; neither consuming
    /// operation accepts another pathname or authority capability.
    ///
    /// # Errors
    ///
    /// Returns [`crash_replace::CrashReplaceError`] for an invalid target,
    /// changed retained binding, or recovery failure.
    pub fn reconcile_file_crash_safe(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> Result<
        retained_crash_replace::RetainedCrashReplaceSession<'lock>,
        crash_replace::CrashReplaceError,
    > {
        let state_relative = self.state_relative(relative).map_err(|source| {
            crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            }
        })?;
        if relative.components().count() != 1 {
            return Err(crash_replace::CrashReplaceError::InvalidPath {
                field: "target",
                path: relative.display().to_string(),
            });
        }
        self.validate()
            .map_err(|source| crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            })?;
        let target = retained_crash_replace::RetainedCrashReplaceTarget::new(
            self.lock,
            self.directory
                .try_clone()
                .map_err(|source| crash_replace::CrashReplaceError::Io {
                    path: self.directory.display_path().join(relative),
                    source: source.to_string(),
                })?,
            state_relative,
        )
        .map_err(|source| crash_replace::CrashReplaceError::Io {
            path: self.directory.display_path().join(relative),
            source: source.to_string(),
        })?;
        retained_crash_replace::reconcile_file_crash_safe_at_owned_retained_target(target, maximum)
    }

    /// Reconcile one crash-replacement target and retain exact present or
    /// Store-minted absence authority for its next replacement.
    ///
    /// # Errors
    ///
    /// Returns [`crash_replace::CrashReplaceError`] for an invalid target,
    /// changed retained binding, or recovery failure.
    pub fn retain_file_crash_safe_expected_leaf(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> Result<RetainedEffectStoreExpectedLeaf<'lock>, crash_replace::CrashReplaceError> {
        let state_relative = self.state_relative(relative).map_err(|source| {
            crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            }
        })?;
        if relative.components().count() != 1 {
            return Err(crash_replace::CrashReplaceError::InvalidPath {
                field: "target",
                path: relative.display().to_string(),
            });
        }
        self.validate()
            .map_err(|source| crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            })?;
        let target = retained_crash_replace::RetainedCrashReplaceTarget::new(
            self.lock,
            self.directory
                .try_clone()
                .map_err(|source| crash_replace::CrashReplaceError::Io {
                    path: self.directory.display_path().join(relative),
                    source: source.to_string(),
                })?,
            state_relative,
        )
        .map_err(|source| crash_replace::CrashReplaceError::Io {
            path: self.directory.display_path().join(relative),
            source: source.to_string(),
        })?;
        retained_crash_replace::retain_file_crash_safe_expected_leaf_at_retained_target(
            &target, maximum,
        )
    }

    /// Recover one crash-replacement target beneath the retained lock parent.
    ///
    /// # Errors
    ///
    /// Returns [`crash_replace::CrashReplaceError`] for an invalid target,
    /// changed retained binding, or recovery failure.
    pub fn recover_file_crash_safe(
        &self,
        relative: &Path,
        maximum: u64,
    ) -> Result<crash_replace::CrashReplaceRecovery, crash_replace::CrashReplaceError> {
        let state_relative = self.state_relative(relative).map_err(|source| {
            crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            }
        })?;
        if relative.components().count() != 1 {
            return Err(crash_replace::CrashReplaceError::InvalidPath {
                field: "target",
                path: relative.display().to_string(),
            });
        }
        self.validate()
            .map_err(|source| crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            })?;
        let target = retained_crash_replace::RetainedCrashReplaceTarget::new(
            self.lock,
            self.directory
                .try_clone()
                .map_err(|source| crash_replace::CrashReplaceError::Io {
                    path: self.directory.display_path().join(relative),
                    source: source.to_string(),
                })?,
            state_relative,
        )
        .map_err(|source| crash_replace::CrashReplaceError::Io {
            path: self.directory.display_path().join(relative),
            source: source.to_string(),
        })?;
        let result =
            retained_crash_replace::recover_file_crash_safe_at_retained_target(&target, maximum);
        if result.is_ok() {
            self.directory
                .sync_root()
                .map_err(|source| crash_replace::CrashReplaceError::Io {
                    path: self.directory.display_path().join(relative),
                    source: source.to_string(),
                })?;
        }
        self.validate()
            .map_err(|source| crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            })?;
        result
    }

    /// Compare-and-swap one crash-replacement target beneath the retained lock
    /// parent and return the exact installed target witness.
    ///
    /// `expected` is exact Store-minted present or absence authority, never a
    /// caller-supplied digest or `None`. The linearization point is the marker
    /// finalizer's closing protocol sweep; the same installed handle is returned
    /// directly without reopening the target pathname.
    ///
    /// # Errors
    ///
    /// Returns [`crash_replace::CrashReplaceError`] for an invalid target,
    /// changed retained binding, CAS mismatch, or durable replacement failure.
    pub fn replace_file_crash_safe(
        &self,
        relative: &Path,
        expected: &mut RetainedEffectStoreExpectedLeaf<'lock>,
        content: &[u8],
        maximum: u64,
    ) -> Result<RetainedEffectStoreLeafWitness<'lock>, crash_replace::CrashReplaceError> {
        let state_relative = self.state_relative(relative).map_err(|source| {
            crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            }
        })?;
        if relative.components().count() != 1 {
            return Err(crash_replace::CrashReplaceError::InvalidPath {
                field: "target",
                path: relative.display().to_string(),
            });
        }
        self.validate()
            .map_err(|source| crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            })?;
        if !expected.is_bound_to(self, relative) {
            return Err(crash_replace::CrashReplaceError::InvalidArgument {
                field: "expected_previous",
                reason: "retained authority is bound to a different lock, root, parent, or leaf"
                    .to_owned(),
            });
        }
        expected
            .revalidate()
            .map_err(|source| crash_replace::CrashReplaceError::Io {
                path: self.directory.display_path().join(relative),
                source: source.to_string(),
            })?;
        let expected_digest = expected.digest().map(str::to_owned);
        let expected_binding = match expected {
            RetainedEffectStoreExpectedLeaf::Present(witness) => {
                retained_crash_replace::RetainedExpectedTarget::Exact {
                    file: &witness.file,
                    identity: &witness.leaf_identity,
                }
            }
            RetainedEffectStoreExpectedLeaf::Absent(_) => {
                retained_crash_replace::RetainedExpectedTarget::Absent
            }
        };
        let installed_parent =
            self.directory
                .try_clone()
                .map_err(|source| crash_replace::CrashReplaceError::Io {
                    path: self.directory.display_path().join(relative),
                    source: source.to_string(),
                })?;
        let target = retained_crash_replace::RetainedCrashReplaceTarget::new(
            self.lock,
            self.directory
                .try_clone()
                .map_err(|source| crash_replace::CrashReplaceError::Io {
                    path: self.directory.display_path().join(relative),
                    source: source.to_string(),
                })?,
            state_relative,
        )
        .map_err(|source| crash_replace::CrashReplaceError::Io {
            path: self.directory.display_path().join(relative),
            source: source.to_string(),
        })?;
        let retained =
            retained_crash_replace::replace_file_crash_safe_at_retained_target_with_witness(
                &target,
                expected_digest.as_deref(),
                expected_binding,
                content,
                maximum,
            )?;
        if retained.result.installed_digest != sha256_content_hash(content) {
            return Err(crash_replace::CrashReplaceError::Protocol {
                reason: "retained crash replacement returned a mismatched installed digest"
                    .to_owned(),
            });
        }
        // Marker finalization returned this exact installed handle after its
        // closing target/protocol sweep. Construct the public witness directly;
        // reopening or revalidating the target pathname here would create a new
        // post-linearization observation rather than preserving that witness.
        Ok(RetainedEffectStoreLeafWitness {
            lock: self.lock,
            parent: installed_parent,
            parent_relative_path: self.directory_relative_path.clone(),
            state_root_identity: self.state_root_identity.clone(),
            parent_identity: self.directory_identity.clone(),
            relative_path: relative.to_path_buf(),
            file: retained.installed_file,
            leaf_identity: retained.installed_identity,
            bytes: content.to_vec(),
            digest: retained.result.installed_digest,
            maximum,
        })
    }
}

impl Drop for EffectStoreLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectStoreLockError {
    InvalidRelativePath {
        path: String,
    },
    ReservedStatePath {
        path: String,
        reserved: ReservedStatePath,
    },
    CreateDir {
        path: PathBuf,
        source: String,
    },
    OpenFile {
        path: PathBuf,
        source: String,
    },
    ProducerBoundary {
        source: producer_quiescence::ProducerBoundaryError,
    },
    Lock {
        path: PathBuf,
        source: String,
    },
    WouldBlock {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectWalCompactionResult {
    pub status: EffectWalCompactionStatus,
    pub retained_records: usize,
    pub dropped_records: usize,
    pub incomplete_transactions: Vec<String>,
    pub reasons: Vec<EffectWalCompactionReason>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectWalCompactionStatus {
    Compacted,
    Noop,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectWalCompactionReason {
    NoWalFile,
    NoClosedRecords,
    ClosedRecordsDropped,
    ProvenanceRecordsRetained,
    StoreLockFailed,
    WalReadFailed,
    WalParseFailed,
    WalWriteFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
struct EffectWalCompactionManifest {
    schema_version: String,
    wal_relative_path: String,
    status: String,
    retained_records: usize,
    dropped_records: usize,
    incomplete_transactions: Vec<String>,
}

#[must_use]
pub fn sha256_content_hash(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    format!("sha256:{digest:x}")
}

/// Opaque cleanup debt left by a successful two-phase replacement.
///
/// The isolated paths are deliberately not exposed: callers retain authority
/// only for the requested destination, while Store maintenance owns any later
/// accounting of exact displaced objects.
#[derive(Debug)]
pub(crate) struct RetainedTwoPhaseCleanupDebt {
    _isolated_paths: Vec<PathBuf>,
}

#[derive(Debug)]
struct RetainedTwoPhaseLeaf {
    path: PathBuf,
    file: File,
    identity: retained_dir::RetainedFileIdentity,
    bytes: Vec<u8>,
}

struct ExpectedRetainedTwoPhasePrevious<'a> {
    file: &'a File,
    identity: &'a retained_dir::RetainedFileIdentity,
    bytes: &'a [u8],
}

/// Install one exact retained authority leaf, then run the caller's idempotent
/// commit validation before and after the final exact-object sweep while both
/// the displaced and installed objects remain retained. A failed validation
/// restores the exact displaced object when
/// possible; otherwise both exact objects remain isolated under Store-created
/// no-replace names. No mutable name is ever unlinked after separate
/// validation. The replacement linearizes only when the second commit
/// validation succeeds after the exact installed, previous, placeholder, root,
/// and caller-owned lock witnesses have all survived the closing sweep; no later
/// pathname reopen contributes to a successful return.
pub(crate) fn replace_retained_file_two_phase<F>(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    content: &[u8],
    commit_validation: F,
) -> io::Result<RetainedTwoPhaseCleanupDebt>
where
    F: FnMut() -> io::Result<()>,
{
    replace_retained_file_two_phase_inner(root, target, None, content, commit_validation)
}

/// Replace one authority leaf only if the namespace still names the caller's
/// exact retained previous handle and bytes. This closes the recovery-to-commit
/// ABA window for callers that derived a new immutable authority record from a
/// retained source object.
pub(crate) fn replace_retained_file_two_phase_from_expected<F>(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    expected_file: &File,
    expected_identity: &retained_dir::RetainedFileIdentity,
    expected_bytes: &[u8],
    content: &[u8],
    commit_validation: F,
) -> io::Result<RetainedTwoPhaseCleanupDebt>
where
    F: FnMut() -> io::Result<()>,
{
    replace_retained_file_two_phase_inner(
        root,
        target,
        Some(ExpectedRetainedTwoPhasePrevious {
            file: expected_file,
            identity: expected_identity,
            bytes: expected_bytes,
        }),
        content,
        commit_validation,
    )
}

fn replace_retained_file_two_phase_inner<F>(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    expected_previous: Option<ExpectedRetainedTwoPhasePrevious<'_>>,
    content: &[u8],
    mut commit_validation: F,
) -> io::Result<RetainedTwoPhaseCleanupDebt>
where
    F: FnMut() -> io::Result<()>,
{
    let authority = root.retain_authority()?;
    let mut previous = retain_optional_two_phase_leaf(root, target)?;
    validate_expected_two_phase_previous(previous.as_ref(), expected_previous.as_ref())?;
    let staged_path = two_phase_quarantine_path(target, "install")?;
    let mut installed = create_two_phase_leaf(root, &staged_path, content)?;
    let mut cleanup_debt = Vec::new();

    if let Some(previous_leaf) = previous.as_mut() {
        let previous_path = two_phase_quarantine_path(target, "previous")?;
        let previous_move_debt = authority
            .rename_file_noreplace_with_validation(
                target,
                &previous_path,
                |directory, source, _| {
                    validate_two_phase_leaf(directory, previous_leaf, source)?;
                    validate_two_phase_leaf(directory, &installed, &installed.path)
                },
            )
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "{error}; uninstalled replacement remains isolated at {}",
                        installed.path.display()
                    ),
                )
            })?;
        cleanup_debt.extend(previous_move_debt.into_paths());
        previous_leaf.path = previous_path;
    }

    // Precommit begins only after an exact Store placeholder occupies target.
    // Its retained handle remains live until either the exact previous object is
    // restored or an error returns with that same placeholder rebound at target.
    let mut placeholder = create_two_phase_target_placeholder(
        &authority,
        root,
        target,
        &installed,
        previous.as_ref(),
        &mut cleanup_debt,
    )?;
    let parked_placeholder_path = two_phase_quarantine_path(target, "target-placeholder")?;
    let placeholder_move_debt = authority
        .rename_file_noreplace_with_validation(
            target,
            &parked_placeholder_path,
            |directory, source, _| {
                validate_two_phase_leaf(directory, &placeholder, source)?;
                validate_two_phase_leaf(directory, &installed, &installed.path)?;
                if let Some(previous_leaf) = previous.as_ref() {
                    validate_two_phase_leaf(directory, previous_leaf, &previous_leaf.path)?;
                }
                Ok(())
            },
        )
        .map_err(|error| {
            let rollback = restore_two_phase_previous_or_placeholder(
                &authority,
                root,
                target,
                previous.as_mut(),
                &placeholder,
            );
            io::Error::new(
                error.kind(),
                format!(
                    "{error}; two-phase target-placeholder parking failed and fail-closed restoration result was {rollback:?}"
                ),
            )
        })?;
    cleanup_debt.extend(placeholder_move_debt.into_paths());
    placeholder.path = parked_placeholder_path;
    validate_two_phase_leaf(root, &placeholder, &placeholder.path)?;

    let install_move_debt = match authority.rename_file_noreplace_with_validation(
        &installed.path,
        target,
        |directory, source, _| {
            validate_two_phase_leaf(directory, &installed, source)?;
            validate_two_phase_handle(&placeholder)?;
            if let Some(previous_leaf) = previous.as_ref() {
                validate_two_phase_leaf(directory, previous_leaf, &previous_leaf.path)?;
            }
            Ok(())
        },
    ) {
        Ok(cleanup_debt) => cleanup_debt,
        Err(install_error) => {
            let rollback = restore_two_phase_previous_or_placeholder(
                &authority,
                root,
                target,
                previous.as_mut(),
                &placeholder,
            );
            return Err(io::Error::new(
                install_error.kind(),
                match rollback {
                    Ok(true) => format!(
                        "{install_error}; exact previous destination was restored and the uninstalled replacement remains isolated at {}",
                        installed.path.display()
                    ),
                    Ok(false) => format!(
                        "{install_error}; exact Store placeholder remains at the previously vacant destination and the uninstalled replacement remains isolated at {}",
                        installed.path.display()
                    ),
                    Err(rollback_error) => format!(
                        "{install_error}; replacement remains isolated at {} and fail-closed previous-or-placeholder restoration failed: {rollback_error}",
                        installed.path.display()
                    ),
                },
            ));
        }
    };
    cleanup_debt.extend(install_move_debt.into_paths());
    installed.path = target.to_path_buf();

    let commit = (|| {
        validate_two_phase_leaf(root, &installed, target)?;
        validate_two_phase_handle(&placeholder)?;
        if let Some(previous_leaf) = previous.as_ref() {
            validate_two_phase_leaf(root, previous_leaf, &previous_leaf.path)?;
        }
        commit_validation()?;
        validate_two_phase_leaf(root, &installed, target)?;
        validate_two_phase_handle(&placeholder)?;
        if let Some(previous_leaf) = previous.as_ref() {
            validate_two_phase_leaf(root, previous_leaf, &previous_leaf.path)?;
        }
        commit_validation()
    })();

    if let Err(commit_error) = commit {
        let rollback = rollback_or_isolate_two_phase_install(
            &authority,
            root,
            target,
            &mut installed,
            previous.as_mut(),
            &placeholder,
        );
        return Err(io::Error::new(
            commit_error.kind(),
            match rollback {
                Ok(true) => format!(
                    "{commit_error}; exact installed destination was isolated and the exact previous destination was restored"
                ),
                Ok(false) => format!(
                    "{commit_error}; exact installed destination was isolated and the exact Store placeholder remains authoritative"
                ),
                Err(rollback_error) => format!(
                    "{commit_error}; final validation failed and retained rollback could preserve only fail-closed cleanup debt: {rollback_error}"
                ),
            },
        ));
    }

    cleanup_debt.push(placeholder.path);
    cleanup_debt.extend(previous.into_iter().map(|previous_leaf| previous_leaf.path));
    Ok(RetainedTwoPhaseCleanupDebt {
        _isolated_paths: cleanup_debt,
    })
}

fn validate_expected_two_phase_previous(
    actual: Option<&RetainedTwoPhaseLeaf>,
    expected: Option<&ExpectedRetainedTwoPhasePrevious<'_>>,
) -> io::Result<()> {
    match (actual, expected) {
        (None, None) => Ok(()),
        (Some(actual), None) => actual.file.metadata().map(|_| ()),
        (Some(actual), Some(expected)) => {
            if retained_dir::RetainedDirectory::identity_of(expected.file)? != *expected.identity
                || actual.identity != *expected.identity
                || actual.bytes.as_slice() != expected.bytes
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained two-phase previous authority does not match the caller's exact witness",
                ));
            }
            let mut retained = expected.file.try_clone()?;
            retained.seek(SeekFrom::Start(0))?;
            let mut bytes = Vec::new();
            retained.read_to_end(&mut bytes)?;
            if bytes.as_slice() != expected.bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained two-phase expected previous bytes changed",
                ));
            }
            Ok(())
        }
        (None, Some(_)) => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "retained two-phase expected previous authority is absent",
        )),
    }
}

fn retain_optional_two_phase_leaf(
    root: &retained_dir::RetainedDirectory,
    path: &Path,
) -> io::Result<Option<RetainedTwoPhaseLeaf>> {
    let mut file = match root.open_leaf_read_delete_rename_authority(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let identity = retained_dir::RetainedDirectory::identity_of(&file)?;
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let retained = RetainedTwoPhaseLeaf {
        path: path.to_path_buf(),
        file,
        identity,
        bytes,
    };
    validate_two_phase_leaf(root, &retained, path)?;
    Ok(Some(retained))
}

fn create_two_phase_leaf(
    root: &retained_dir::RetainedDirectory,
    path: &Path,
    content: &[u8],
) -> io::Result<RetainedTwoPhaseLeaf> {
    let mut file = root.open_leaf_write_new_authority(path)?;
    if let Err(error) = file.write_all(content).and_then(|()| file.sync_all()) {
        return Err(io::Error::new(
            error.kind(),
            format!(
                "{error}; partial two-phase replacement remains isolated at {}",
                path.display()
            ),
        ));
    }
    if let Err(error) = sync_two_phase_parent(root, path) {
        return Err(io::Error::new(
            error.kind(),
            format!(
                "{error}; durable two-phase replacement remains isolated at {}",
                path.display()
            ),
        ));
    }
    let retained = RetainedTwoPhaseLeaf {
        path: path.to_path_buf(),
        identity: retained_dir::RetainedDirectory::identity_of(&file).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "{error}; two-phase replacement remains isolated at {}",
                    path.display()
                ),
            )
        })?,
        file,
        bytes: content.to_vec(),
    };
    validate_two_phase_leaf(root, &retained, path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "{error}; two-phase replacement remains isolated at {}",
                path.display()
            ),
        )
    })?;
    Ok(retained)
}

fn validate_two_phase_handle(retained: &RetainedTwoPhaseLeaf) -> io::Result<()> {
    if retained_dir::RetainedDirectory::identity_of(&retained.file)? == retained.identity {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained two-phase file handle changed identity",
        ))
    }
}

fn validate_two_phase_leaf(
    root: &retained_dir::RetainedDirectory,
    retained: &RetainedTwoPhaseLeaf,
    current_path: &Path,
) -> io::Result<()> {
    validate_two_phase_handle(retained)?;
    let mut current =
        root.open_leaf_read(current_path, retained_dir::RetainedLeafPolicy::Authority)?;
    if retained_dir::RetainedDirectory::identity_of(&current)? != retained.identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained two-phase namespace changed identity",
        ));
    }
    current.seek(SeekFrom::Start(0))?;
    let mut actual = Vec::new();
    current.read_to_end(&mut actual)?;
    if actual != retained.bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained two-phase file bytes changed",
        ));
    }
    Ok(())
}

fn create_two_phase_target_placeholder(
    authority: &retained_dir::RetainedAuthorityDirectory<'_>,
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    installed: &RetainedTwoPhaseLeaf,
    previous: Option<&RetainedTwoPhaseLeaf>,
    cleanup_debt: &mut Vec<PathBuf>,
) -> io::Result<RetainedTwoPhaseLeaf> {
    const PLACEHOLDER_ATTEMPTS: usize = 32;
    for _ in 0..PLACEHOLDER_ATTEMPTS {
        match create_two_phase_leaf(root, target, b"") {
            Ok(placeholder) => {
                validate_two_phase_leaf(root, installed, &installed.path)?;
                if let Some(previous) = previous {
                    validate_two_phase_leaf(root, previous, &previous.path)?;
                }
                validate_two_phase_leaf(root, &placeholder, target)?;
                return Ok(placeholder);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let debt = authority.isolate_name(target).map_err(|isolation| {
                    io::Error::other(
                        format!(
                            "{error}; non-exact two-phase target occupant isolation failed before placeholder installation: {isolation}"
                        ),
                    )
                })?;
                cleanup_debt.extend(debt.into_paths());
            }
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "bounded two-phase target-placeholder installation was continuously repopulated",
    ))
}

fn restore_two_phase_previous_or_placeholder(
    authority: &retained_dir::RetainedAuthorityDirectory<'_>,
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    previous: Option<&mut RetainedTwoPhaseLeaf>,
    placeholder: &RetainedTwoPhaseLeaf,
) -> io::Result<bool> {
    let _occupant_debt = authority.force_retained_placeholder_at(
        &placeholder.file,
        &placeholder.identity,
        target,
    )?;
    validate_two_phase_leaf(root, placeholder, target)?;
    let Some(previous) = previous else {
        return Ok(false);
    };

    let rollback_placeholder_path = two_phase_quarantine_path(target, "rollback-placeholder")?;
    let _placeholder_debt = authority.rename_file_noreplace_with_validation(
        target,
        &rollback_placeholder_path,
        |directory, source, _| validate_two_phase_leaf(directory, placeholder, source),
    )?;
    validate_two_phase_handle(placeholder)?;
    match authority.publish_retained_handle_noreplace(&previous.file, &previous.identity, target) {
        Ok(()) => {
            previous.path = target.to_path_buf();
            validate_two_phase_leaf(root, previous, target)?;
            Ok(true)
        }
        Err(previous_error) => {
            let _failed_restore_debt = authority.force_retained_placeholder_at(
                &placeholder.file,
                &placeholder.identity,
                target,
            )?;
            validate_two_phase_leaf(root, placeholder, target)?;
            Err(io::Error::other(
                format!(
                    "exact previous-object restoration failed ({previous_error}); the retained exact Store placeholder remains authoritative at {}",
                    target.display()
                ),
            ))
        }
    }
}

fn rollback_or_isolate_two_phase_install(
    authority: &retained_dir::RetainedAuthorityDirectory<'_>,
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    installed: &mut RetainedTwoPhaseLeaf,
    previous: Option<&mut RetainedTwoPhaseLeaf>,
    placeholder: &RetainedTwoPhaseLeaf,
) -> io::Result<bool> {
    // The installed handle remains retained while rollback first atomically
    // forces the exact Store placeholder over whatever currently occupies target.
    // Any exact installed object or non-exact substitute displaced by that step
    // remains discoverable Store cleanup debt.
    validate_two_phase_handle(installed)?;

    // Restoration is handle-bound. On any previous-object publication failure,
    // restore_two_phase_previous_or_placeholder rebinds the retained exact Store
    // placeholder before returning the error, so no error return intentionally
    // leaves an attacker-controlled target authoritative.
    restore_two_phase_previous_or_placeholder(authority, root, target, previous, placeholder)
}

fn sync_two_phase_parent(root: &retained_dir::RetainedDirectory, path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    if parent.as_os_str().is_empty() {
        root.sync_root()
    } else {
        root.sync_directory(parent)
    }
}

fn two_phase_quarantine_path(target: &Path, purpose: &str) -> io::Result<PathBuf> {
    let mut nonce = [0_u8; 16];
    getrandom::fill(&mut nonce).map_err(|error| {
        io::Error::other(format!(
            "two-phase replacement nonce generation failed: {error}"
        ))
    })?;
    let nonce = u128::from_le_bytes(nonce);
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    Ok(parent.join(format!(
        ".forge-retained-two-phase-{purpose}-{}-{nonce}.quarantine",
        std::process::id()
    )))
}

/// Block until an exclusive [`EffectStoreLock`] is acquired on
/// `lock_relative_path` under `root`.
///
/// # Errors
///
/// Returns [`EffectStoreLockError`] if the lock path is invalid, its parent
/// directory cannot be created, or the lock file cannot be opened.
pub fn acquire_effect_store_lock(
    root: impl AsRef<Path>,
    lock_relative_path: &str,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    let root = root.as_ref();
    let state_root = effect_boundary_state_root(root, lock_relative_path);
    let boundary = producer_quiescence::admit_effect_producer(&state_root, false)
        .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    let boundary = producer_quiescence::BoundaryLease::from_boundary(&boundary, &state_root)
        .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    acquire_effect_store_lock_inner(root, lock_relative_path, false, false, boundary, true)
}

/// Acquire an effect authority while retaining an already-held typed producer boundary.
///
/// This is the only path backup orchestration may use while host quiescence is held.
pub fn acquire_effect_store_lock_under_boundary(
    boundary: &impl producer_quiescence::ProducerBoundary,
    root: impl AsRef<Path>,
    lock_relative_path: &str,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    let root = root.as_ref();
    let state_root = effect_boundary_state_root(root, lock_relative_path);
    let boundary = producer_quiescence::BoundaryLease::from_boundary(boundary, &state_root)
        .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    acquire_effect_store_lock_inner(root, lock_relative_path, false, true, boundary, true)
}

/// Try to acquire an exclusive [`EffectStoreLock`] on `lock_relative_path`
/// under `root` without blocking.
///
/// # Errors
///
/// Returns [`EffectStoreLockError`] (typically `LockHeld`) if the lock is
/// currently held by another process, or for the same reasons as
/// [`acquire_effect_store_lock`].
pub fn try_acquire_effect_store_lock(
    root: impl AsRef<Path>,
    lock_relative_path: &str,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    let root = root.as_ref();
    let state_root = effect_boundary_state_root(root, lock_relative_path);
    let boundary =
        producer_quiescence::admit_effect_producer(&state_root, true).map_err(|source| {
            if matches!(
                source,
                producer_quiescence::ProducerBoundaryError::EffectAuthorityHeld { .. }
            ) {
                EffectStoreLockError::WouldBlock {
                    path: root.join(lock_relative_path),
                }
            } else {
                EffectStoreLockError::ProducerBoundary { source }
            }
        })?;
    let boundary = producer_quiescence::BoundaryLease::from_boundary(&boundary, &state_root)
        .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    acquire_effect_store_lock_inner(root, lock_relative_path, true, false, boundary, true)
}

/// Validate and revalidate one file-backed effect while retaining the exact
/// caller-supplied effect-store lock. This function never appends to the WAL
/// and never applies a write.
///
/// # Failure reporting
///
/// This API returns a typed blocked result; lock-scope,
/// contract, freshness, payload, and path failures are accumulated in
/// `reasons` and `diagnostics` for agent self-correction.
#[must_use]
pub fn preflight_file_effect_transaction_under_lock(
    root: impl AsRef<Path>,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
) -> EffectPreflightResult {
    let root = root.as_ref();
    let effect_contract = &effect.tool_effect_contract;
    let validation = validate_tool_effect(effect);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();
    let mut reasons = Vec::new();
    let mut diagnostics = Vec::new();

    if let Err(error) = validate_effect_lock_scope(root, effect_lock, expected_lock_relative_path) {
        reasons.push(EffectApplicationReason::StoreLockFailed);
        diagnostics.push(format!("effect preflight lock scope mismatch: {error}"));
    }
    if validation_error_count > 0 {
        reasons.push(EffectApplicationReason::EffectValidationErrors);
    }
    validate_file_backed_reads_retained(&effect_lock.root, effect, &mut reasons, &mut diagnostics);
    let prepared = prepare_file_writes(
        root,
        &effect_lock.root,
        effect,
        payloads,
        &mut reasons,
        &mut diagnostics,
    );
    let mut metadata_records = Vec::new();
    if reasons.is_empty() {
        if let Some(mut writes) = prepared {
            if revalidate_prepared_writes_retained(
                &effect_lock.root,
                &mut writes,
                &mut reasons,
                &mut diagnostics,
            ) {
                metadata_records = effect_target_metadata_records(effect, &writes);
            }
        } else {
            reasons.push(EffectApplicationReason::InternalInvariant);
            diagnostics.push(
                "internal invariant: preflight writes missing despite empty reasons".to_owned(),
            );
        }
    }

    EffectPreflightResult {
        status: if reasons.is_empty() {
            EffectPreflightStatus::Ready
        } else {
            EffectPreflightStatus::Blocked
        },
        effect_id: effect_contract.id.clone(),
        metadata_records,
        reasons,
        diagnostics,
        validation_error_count,
        validation_warning_count,
    }
}

pub fn apply_file_effect_transaction(
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
) -> EffectApplicationResult {
    let root = root.as_ref();
    let boundary = match producer_quiescence::admit_effect_producer(root, false) {
        Ok(boundary) => boundary,
        Err(error) => {
            return blocked_effect_application_result(
                effect.tool_effect_contract.id.clone(),
                vec![EffectApplicationReason::StoreLockFailed],
                vec![format!("producer boundary failed: {error}")],
                0,
                0,
            );
        }
    };
    let publication_root = match retained_effect_publication_root(&boundary, root, "") {
        Ok(root) => root,
        Err(error) => {
            return blocked_effect_application_result(
                effect.tool_effect_contract.id.clone(),
                vec![EffectApplicationReason::StoreLockFailed],
                vec![error],
                0,
                0,
            );
        }
    };
    let effect_contract = &effect.tool_effect_contract;
    let validation = validate_tool_effect(effect);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();
    let mut reasons = Vec::new();
    let mut diagnostics = Vec::new();

    if validation_error_count > 0 {
        reasons.push(EffectApplicationReason::EffectValidationErrors);
    }

    validate_file_backed_reads_retained(&publication_root, effect, &mut reasons, &mut diagnostics);
    let prepared = prepare_file_writes(
        root,
        &publication_root,
        effect,
        payloads,
        &mut reasons,
        &mut diagnostics,
    );

    if !reasons.is_empty() {
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }

    let Some(mut writes) = prepared else {
        debug_assert!(
            !reasons.is_empty(),
            "prepared writes missing when there are no preflight reasons"
        );
        reasons.push(EffectApplicationReason::InternalInvariant);
        diagnostics.push(
            "internal invariant: prepared writes missing despite empty preflight reasons"
                .to_string(),
        );
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    };
    if !revalidate_prepared_writes_retained(
        &publication_root,
        &mut writes,
        &mut reasons,
        &mut diagnostics,
    ) {
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }
    let originals = match capture_originals_retained(&publication_root, &mut writes) {
        Ok(originals) => originals,
        Err(error) => {
            reasons.push(EffectApplicationReason::ApplyFailed);
            diagnostics.push(format!("capture retained before image failed: {error}"));
            return blocked_effect_application_result(
                effect_contract.id.clone(),
                reasons,
                diagnostics,
                validation_error_count,
                validation_warning_count,
            );
        }
    };
    let mut applied_refs = Vec::new();

    for (write, original) in writes.iter().zip(originals.iter()) {
        if let Err(error) = apply_prepared_write_for_publication(&publication_root, write, original)
        {
            reasons.push(EffectApplicationReason::ApplyFailed);
            diagnostics.push(format!("apply {} failed: {error}", write.target.display()));
            let rollback =
                rollback_originals_for_publication(&publication_root, &originals, &mut diagnostics);
            if !rollback {
                reasons.push(EffectApplicationReason::RollbackFailed);
            }
            return EffectApplicationResult {
                status: if rollback {
                    EffectApplicationStatus::RolledBack
                } else {
                    EffectApplicationStatus::RollbackFailed
                },
                effect_id: effect_contract.id.clone(),
                applied_refs,
                metadata_records: Vec::new(),
                rolled_back: rollback,
                reasons,
                diagnostics,
                validation_error_count,
                validation_warning_count,
            };
        }
        applied_refs.push(write.reference.clone());
    }

    reasons.push(EffectApplicationReason::Applied);
    EffectApplicationResult {
        status: EffectApplicationStatus::Applied,
        effect_id: effect_contract.id.clone(),
        applied_refs,
        metadata_records: effect_target_metadata_records(effect, &writes),
        rolled_back: false,
        reasons,
        diagnostics,
        validation_error_count,
        validation_warning_count,
    }
}

pub fn apply_file_effect_transaction_with_wal_lock(
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    lock_relative_path: &str,
    tx_id: impl Into<String>,
) -> EffectApplicationResult {
    apply_file_effect_transaction_with_wal_lock_with_durability(
        root,
        effect,
        payloads,
        wal_relative_path,
        lock_relative_path,
        tx_id,
        WalDurability::default(),
    )
}

/// [`apply_file_effect_transaction_with_wal_lock`] with an explicit
/// [`WalDurability`] knob. See ADR-0009.
pub fn apply_file_effect_transaction_with_wal_lock_with_durability(
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    lock_relative_path: &str,
    tx_id: impl Into<String>,
    durability: WalDurability,
) -> EffectApplicationResult {
    let root = root.as_ref();
    match acquire_effect_store_lock(root, lock_relative_path) {
        Ok(lock) => {
            if let Err(error) = validate_effect_lock_scope(root, &lock, lock_relative_path) {
                return blocked_effect_application_result(
                    effect.tool_effect_contract.id.clone(),
                    vec![EffectApplicationReason::StoreLockFailed],
                    vec![error.to_string()],
                    0,
                    0,
                );
            }
            let tx_id = tx_id.into();
            apply_file_effect_transaction_with_wal_inner(
                root,
                effect,
                payloads,
                wal_relative_path,
                &tx_id,
                durability,
                None,
                &lock.root,
            )
        }
        Err(error) => EffectApplicationResult {
            status: EffectApplicationStatus::Blocked,
            effect_id: effect.tool_effect_contract.id.clone(),
            applied_refs: Vec::new(),
            metadata_records: Vec::new(),
            rolled_back: false,
            reasons: vec![EffectApplicationReason::StoreLockFailed],
            diagnostics: vec![format!("effect store lock failed: {error}")],
            validation_error_count: 0,
            validation_warning_count: 0,
        },
    }
}

/// Apply one WAL-backed effect transaction while retaining the caller's exact
/// effect-producer or host-quiescence boundary.
#[allow(clippy::too_many_arguments)]
pub fn apply_file_effect_transaction_with_wal_lock_with_durability_under_boundary(
    boundary: &impl producer_quiescence::ProducerBoundary,
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    lock_relative_path: &str,
    tx_id: impl Into<String>,
    durability: WalDurability,
) -> EffectApplicationResult {
    let root = root.as_ref();
    match acquire_effect_store_lock_under_boundary(boundary, root, lock_relative_path) {
        Ok(lock) => {
            if let Err(error) = validate_effect_lock_scope(root, &lock, lock_relative_path) {
                return blocked_effect_application_result(
                    effect.tool_effect_contract.id.clone(),
                    vec![EffectApplicationReason::StoreLockFailed],
                    vec![error.to_string()],
                    0,
                    0,
                );
            }
            let tx_id = tx_id.into();
            apply_file_effect_transaction_with_wal_inner(
                root,
                effect,
                payloads,
                wal_relative_path,
                &tx_id,
                durability,
                None,
                &lock.root,
            )
        }
        Err(error) => blocked_effect_application_result(
            effect.tool_effect_contract.id.clone(),
            vec![EffectApplicationReason::StoreLockFailed],
            vec![format!("effect store lock failed: {error}")],
            0,
            0,
        ),
    }
}

#[instrument(skip_all, fields(effect_id = %effect.tool_effect_contract.id.0, tx_id = tracing::field::Empty), level = "info")]
pub fn apply_file_effect_transaction_with_wal(
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    tx_id: impl Into<String>,
) -> EffectApplicationResult {
    apply_file_effect_transaction_with_wal_with_durability(
        root,
        effect,
        payloads,
        wal_relative_path,
        tx_id,
        WalDurability::default(),
    )
}

/// [`apply_file_effect_transaction_with_wal`] with an explicit [`WalDurability`] knob.
///
/// `durability` controls the four descriptor-relative WAL appends this function
/// performs (begin, before-image, write-applied, commit). Rollback paths hard-code
/// [`WalDurability::SyncOnAppend`]
/// because losing a rollback marker would corrupt the next recovery pass.
/// See ADR-0009.
#[instrument(skip_all, fields(effect_id = %effect.tool_effect_contract.id.0, tx_id = tracing::field::Empty), level = "info")]
pub fn apply_file_effect_transaction_with_wal_with_durability(
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    tx_id: impl Into<String>,
    durability: WalDurability,
) -> EffectApplicationResult {
    let root = root.as_ref();
    let state_root = effect_boundary_state_root(root, wal_relative_path);
    let boundary = match producer_quiescence::admit_effect_producer(&state_root, false) {
        Ok(boundary) => boundary,
        Err(error) => {
            return blocked_effect_application_result(
                effect.tool_effect_contract.id.clone(),
                vec![EffectApplicationReason::StoreLockFailed],
                vec![format!("producer boundary failed: {error}")],
                0,
                0,
            );
        }
    };
    let publication_root =
        match retained_effect_publication_root(&boundary, root, wal_relative_path) {
            Ok(root) => root,
            Err(error) => {
                return blocked_effect_application_result(
                    effect.tool_effect_contract.id.clone(),
                    vec![EffectApplicationReason::StoreLockFailed],
                    vec![error],
                    0,
                    0,
                );
            }
        };
    let tx_id = tx_id.into();
    apply_file_effect_transaction_with_wal_inner(
        root,
        effect,
        payloads,
        wal_relative_path,
        &tx_id,
        durability,
        None,
        &publication_root,
    )
}

/// Commit one provenance-bound file effect while the caller retains the exact
/// canonical effect lock. Provenance and replay binding are durably embedded in
/// the `begin` record before any project write. Durability is intentionally
/// fixed to [`WalDurability::SyncOnAppend`] for this authority boundary.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn apply_file_effect_transaction_with_provenance_under_lock(
    root: impl AsRef<Path>,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    tx_id: impl Into<String>,
    provenance: EffectExecutionProvenance,
    replay_binding: EffectReplayCommitBinding,
) -> EffectApplicationResult {
    let root = root.as_ref();
    let effect_id = effect.tool_effect_contract.id.clone();
    let mut diagnostics = Vec::new();
    if let Err(error) = validate_effect_lock_scope(root, effect_lock, expected_lock_relative_path) {
        diagnostics.push(error.to_string());
        return blocked_effect_application_result(
            effect_id,
            vec![EffectApplicationReason::StoreLockFailed],
            diagnostics,
            0,
            0,
        );
    }
    if let Err(error) = provenance.verify() {
        diagnostics.push(error.to_string());
        return blocked_effect_application_result(
            effect_id,
            vec![EffectApplicationReason::ExecutionProvenanceInvalid],
            diagnostics,
            0,
            0,
        );
    }
    if let Err(reason) = validate_effect_replay_binding(&replay_binding) {
        diagnostics.push(reason);
        return blocked_effect_application_result(
            effect_id,
            vec![EffectApplicationReason::ExecutionProvenanceInvalid],
            diagnostics,
            0,
            0,
        );
    }
    let tx_id = tx_id.into();
    apply_file_effect_transaction_with_wal_inner(
        root,
        effect,
        payloads,
        wal_relative_path,
        &tx_id,
        WalDurability::SyncOnAppend,
        Some((provenance, replay_binding)),
        &effect_lock.root,
    )
}

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all, fields(effect_id = %effect.tool_effect_contract.id.0, tx_id = tracing::field::Empty), level = "info")]
fn apply_file_effect_transaction_with_wal_inner(
    root: &Path,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    tx_id: &str,
    durability: WalDurability,
    prepared_authority: Option<(EffectExecutionProvenance, EffectReplayCommitBinding)>,
    publication_root: &retained_dir::RetainedDirectory,
) -> EffectApplicationResult {
    // Record the resolved tx_id in the parent span so callers filtering by
    // transaction id can find this operation in the trace.
    tracing::Span::current().record("tx_id", tx_id);
    let effect_contract = &effect.tool_effect_contract;
    let validation = validate_tool_effect(effect);
    let validation_error_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .count();
    let validation_warning_count = validation
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .count();
    let mut reasons = Vec::new();
    let mut diagnostics = Vec::new();

    if validation_error_count > 0 {
        reasons.push(EffectApplicationReason::EffectValidationErrors);
    }
    validate_file_backed_reads_retained(publication_root, effect, &mut reasons, &mut diagnostics);
    let prepared = prepare_file_writes(
        root,
        publication_root,
        effect,
        payloads,
        &mut reasons,
        &mut diagnostics,
    );
    if !reasons.is_empty() {
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }

    let Some(mut writes) = prepared else {
        debug_assert!(
            !reasons.is_empty(),
            "prepared writes missing when there are no preflight reasons"
        );
        reasons.push(EffectApplicationReason::InternalInvariant);
        diagnostics.push(
            "internal invariant: prepared writes missing despite empty preflight reasons"
                .to_string(),
        );
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    };
    let revalidated = revalidate_prepared_writes_retained(
        publication_root,
        &mut writes,
        &mut reasons,
        &mut diagnostics,
    );
    if !revalidated {
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }
    let originals = match capture_originals_retained(publication_root, &mut writes) {
        Ok(originals) => originals,
        Err(error) => {
            reasons.push(EffectApplicationReason::ApplyFailed);
            diagnostics.push(format!("capture retained before image failed: {error}"));
            return blocked_effect_application_result(
                effect_contract.id.clone(),
                reasons,
                diagnostics,
                validation_error_count,
                validation_warning_count,
            );
        }
    };
    let begin_record = prepared_authority.map_or_else(
        || EffectWalRecord::begin(tx_id, effect_contract.id.clone()),
        |(provenance, replay_binding)| {
            EffectWalRecord::begin_with_authority(
                tx_id,
                effect_contract.id.clone(),
                provenance,
                replay_binding,
            )
        },
    );
    if append_effect_wal_record_for_publication(
        publication_root,
        root,
        wal_relative_path,
        begin_record,
        durability,
    )
    .is_err()
    {
        reasons.push(EffectApplicationReason::WalAppendFailed);
        diagnostics.push("failed to append WAL begin record".to_string());
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }

    #[cfg(test)]
    pause_after_durable_effect_begin_for_test(durability);
    let mut applied_refs = Vec::new();
    for (write, original) in writes.iter().zip(originals.iter()) {
        if append_effect_wal_record_for_publication(
            publication_root,
            root,
            wal_relative_path,
            EffectWalRecord::before_image(tx_id, effect_contract.id.clone(), write, original),
            durability,
        )
        .is_err()
        {
            reasons.push(EffectApplicationReason::WalAppendFailed);
            diagnostics.push(format!(
                "failed to append WAL before image {}",
                write.reference
            ));
            let rollback =
                rollback_originals_for_publication(publication_root, &originals, &mut diagnostics);
            return EffectApplicationResult {
                status: if rollback {
                    EffectApplicationStatus::RolledBack
                } else {
                    EffectApplicationStatus::RollbackFailed
                },
                effect_id: effect_contract.id.clone(),
                applied_refs,
                metadata_records: Vec::new(),
                rolled_back: rollback,
                reasons,
                diagnostics,
                validation_error_count,
                validation_warning_count,
            };
        }

        if let Err(error) = apply_prepared_write_for_publication(publication_root, write, original)
        {
            reasons.push(EffectApplicationReason::ApplyFailed);
            diagnostics.push(format!("apply {} failed: {error}", write.target.display()));
            return rollback_wal_transaction_result(RollbackWalTransaction {
                root,
                publication_root,
                wal_relative_path,
                tx_id,
                effect_id: effect_contract.id.clone(),
                originals: &originals,
                applied_refs,
                reasons,
                diagnostics,
                validation_error_count,
                validation_warning_count,
            });
        }

        if let Err(error) = append_effect_wal_record_for_publication(
            publication_root,
            root,
            wal_relative_path,
            EffectWalRecord::write_applied(tx_id, effect, write),
            durability,
        ) {
            reasons.push(EffectApplicationReason::WalAppendFailed);
            diagnostics.push(format!(
                "failed to append WAL write-applied {}: {error}",
                write.reference
            ));
            return rollback_wal_transaction_result(RollbackWalTransaction {
                root,
                publication_root,
                wal_relative_path,
                tx_id,
                effect_id: effect_contract.id.clone(),
                originals: &originals,
                applied_refs,
                reasons,
                diagnostics,
                validation_error_count,
                validation_warning_count,
            });
        }
        applied_refs.push(write.reference.clone());
    }

    if append_effect_wal_record_for_publication(
        publication_root,
        root,
        wal_relative_path,
        EffectWalRecord::stage(tx_id, effect_contract.id.clone(), EffectWalStage::Commit),
        durability,
    )
    .is_err()
    {
        reasons.push(EffectApplicationReason::WalAppendFailed);
        diagnostics.push("failed to append WAL commit record".to_string());
        return rollback_wal_transaction_result(RollbackWalTransaction {
            root,
            publication_root,
            wal_relative_path,
            tx_id,
            effect_id: effect_contract.id.clone(),
            originals: &originals,
            applied_refs,
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        });
    }

    reasons.push(EffectApplicationReason::Applied);
    EffectApplicationResult {
        status: EffectApplicationStatus::Applied,
        effect_id: effect_contract.id.clone(),
        applied_refs,
        metadata_records: effect_target_metadata_records(effect, &writes),
        rolled_back: false,
        reasons,
        diagnostics,
        validation_error_count,
        validation_warning_count,
    }
}

/// Repair only a missing newline after a complete final record or an
/// incomplete final JSON record while the exact effect lock is retained.
/// Interior corruption is never rewritten.
///
/// # Errors
///
/// Returns [`EffectReplayReconciliationError`] for lock/path/read/repair
/// failure.
pub fn repair_effect_wal_tail_under_lock(
    root: impl AsRef<Path>,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    wal_relative_path: &str,
) -> Result<bool, EffectReplayReconciliationError> {
    let root = root.as_ref();
    validate_effect_lock_scope(root, effect_lock, expected_lock_relative_path)?;
    let wal_relative = reconciliation_relative_path("wal_relative_path", wal_relative_path)?;
    let wal_path = root.join(&wal_relative);
    if !authority_leaf_exists(&effect_lock.root, &wal_relative).map_err(|source| {
        EffectReplayReconciliationError::WalRead {
            path: wal_path.clone(),
            source: source.to_string(),
        }
    })? {
        return Ok(false);
    }
    repair_effect_wal_tail_retained(&effect_lock.root, &wal_relative, &wal_path)
}

/// Inspect committed provenance-bound effect transactions that still require
/// replay completion. The exact effect lock must already be held. Any malformed
/// integrated transaction fails the whole inspection closed.
///
/// # Errors
///
/// Returns [`EffectReplayReconciliationError`] for lock-scope mismatch, WAL
/// read/parse failure, corrupt provenance, or an inconsistent transaction.
pub fn pending_effect_replay_commits_under_lock(
    root: impl AsRef<Path>,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    wal_relative_path: &str,
) -> Result<Vec<PendingEffectReplayCommit>, EffectReplayReconciliationError> {
    let root = root.as_ref();
    validate_effect_lock_scope(root, effect_lock, expected_lock_relative_path)?;
    let wal_relative = reconciliation_relative_path("wal_relative_path", wal_relative_path)?;
    let wal_path = root.join(&wal_relative);
    if !authority_leaf_exists(&effect_lock.root, &wal_relative).map_err(|source| {
        EffectReplayReconciliationError::WalRead {
            path: wal_path.clone(),
            source: source.to_string(),
        }
    })? {
        return Ok(Vec::new());
    }
    let _ = repair_effect_wal_tail_retained(&effect_lock.root, &wal_relative, &wal_path)?;
    let records =
        read_effect_wal_records_strict_retained(&effect_lock.root, &wal_relative, &wal_path)?;
    project_pending_effect_replay_commits(&records)
}

/// Append the durable effect-WAL acknowledgement for a consumed replay
/// reservation while retaining the exact effect lock.
///
/// # Errors
///
/// Returns [`EffectReplayReconciliationError`] for a mismatched lock/binding or
/// durable WAL append failure.
#[allow(clippy::too_many_arguments)]
pub fn append_effect_replay_completion_under_lock(
    root: impl AsRef<Path>,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    wal_relative_path: &str,
    tx_id: &str,
    effect_id: &StableId,
    replay_binding: &EffectReplayCommitBinding,
    replay_result: &replay_wal::ReplayConsumeResult,
    recovered: bool,
) -> Result<EffectReplayCompletionResult, EffectReplayReconciliationError> {
    let root = root.as_ref();
    validate_effect_lock_scope(root, effect_lock, expected_lock_relative_path)?;
    if tx_id.trim().is_empty() {
        return Err(EffectReplayReconciliationError::ConflictingTransaction {
            tx_id: tx_id.to_owned(),
            reason: "transaction id must not be blank".to_owned(),
        });
    }
    validate_effect_replay_binding(replay_binding).map_err(|reason| {
        EffectReplayReconciliationError::InvalidReplayBinding {
            tx_id: tx_id.to_owned(),
            reason,
        }
    })?;
    let consumed_revision = replay_binding
        .reservation_revision
        .checked_add(1)
        .ok_or_else(|| EffectReplayReconciliationError::InvalidReplayBinding {
            tx_id: tx_id.to_owned(),
            reason: "reservation revision overflow".to_owned(),
        })?;
    let reservation = &replay_result.reservation;
    if reservation.key_hash != replay_binding.key_hash
        || reservation.intent_digest != replay_binding.intent_digest
        || reservation.commit_digest != replay_binding.commit_digest
        || reservation.state != replay_wal::ReplayReservationState::Consumed
        || reservation.revision != consumed_revision
    {
        return Err(EffectReplayReconciliationError::InvalidReplayBinding {
            tx_id: tx_id.to_owned(),
            reason: "consume result does not match the effect-WAL replay binding".to_owned(),
        });
    }
    let completion = EffectReplayCompletion {
        key_hash: replay_binding.key_hash.clone(),
        reservation_revision: replay_binding.reservation_revision,
        consumed_revision,
        consumed_seq: replay_result.seq,
        recovered,
    };
    let wal_relative = reconciliation_relative_path("wal_relative_path", wal_relative_path)?;
    let wal_path = root.join(&wal_relative);
    if !authority_leaf_exists(&effect_lock.root, &wal_relative).map_err(|source| {
        EffectReplayReconciliationError::WalRead {
            path: wal_path.clone(),
            source: source.to_string(),
        }
    })? {
        return Err(EffectReplayReconciliationError::ConflictingTransaction {
            tx_id: tx_id.to_owned(),
            reason: "effect WAL is missing after replay consume".to_owned(),
        });
    }
    let _ = repair_effect_wal_tail_retained(&effect_lock.root, &wal_relative, &wal_path)?;
    let records =
        read_effect_wal_records_strict_retained(&effect_lock.root, &wal_relative, &wal_path)?;
    let pending = project_pending_effect_replay_commits(&records)?;
    let exact_commit = pending.iter().any(|candidate| {
        candidate.tx_id == tx_id
            && candidate.effect_id == *effect_id
            && candidate.replay_binding == *replay_binding
    });
    if !exact_commit {
        return Err(EffectReplayReconciliationError::ConflictingTransaction {
            tx_id: tx_id.to_owned(),
            reason: "no exact committed effect is pending replay completion".to_owned(),
        });
    }
    append_effect_wal_record_for_publication(
        &effect_lock.root,
        root,
        wal_relative_path,
        EffectWalRecord::replay_consumed(
            tx_id,
            effect_id.clone(),
            replay_binding.clone(),
            completion.clone(),
        ),
        WalDurability::SyncOnAppend,
    )
    .map_err(|error| EffectReplayReconciliationError::WalAppend {
        path: wal_path.clone(),
        source: error.to_string(),
    })?;
    Ok(EffectReplayCompletionResult {
        wal_path,
        tx_id: tx_id.to_owned(),
        completion,
    })
}

pub fn recover_effect_wal_with_lock(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    lock_relative_path: &str,
) -> EffectWalRecoveryResult {
    let root = root.as_ref();
    match acquire_effect_store_lock(root, lock_relative_path) {
        Ok(lock) => {
            if let Err(error) = validate_effect_lock_scope(root, &lock, lock_relative_path) {
                return EffectWalRecoveryResult {
                    status: EffectWalRecoveryStatus::RecoveryFailed,
                    recovered_transactions: Vec::new(),
                    reasons: vec![EffectWalRecoveryReason::StoreLockFailed],
                    diagnostics: vec![error.to_string()],
                };
            }
            recover_effect_wal_under_publication_root(root, &lock.root, wal_relative_path)
        }
        Err(error) => EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::StoreLockFailed],
            diagnostics: vec![format!("effect store lock failed: {error}")],
        },
    }
}

/// Recover an effect WAL while retaining the caller's exact effect lock.
/// A stale or mismatched root/lock pair returns a fail-closed recovery result.
pub fn recover_effect_wal_under_lock(
    root: impl AsRef<Path>,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
    wal_relative_path: &str,
) -> EffectWalRecoveryResult {
    let root = root.as_ref();
    if let Err(error) = validate_effect_lock_scope(root, effect_lock, expected_lock_relative_path) {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::StoreLockFailed],
            diagnostics: vec![error.to_string()],
        };
    }
    recover_effect_wal_under_publication_root(root, &effect_lock.root, wal_relative_path)
}

fn recover_effect_wal_under_publication_root(
    root: &Path,
    publication_root: &retained_dir::RetainedDirectory,
    wal_relative_path: &str,
) -> EffectWalRecoveryResult {
    if let Some(reserved) = reserved_state_path(wal_relative_path) {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::ReservedStatePath],
            diagnostics: vec![format!(
                "WAL path is reserved for EventLog TCB: {reserved:?}"
            )],
        };
    }
    let Some(wal_relative) = normalized_effect_relative_path(wal_relative_path) else {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::WalReadFailed],
            diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
        };
    };
    let wal_path = root.join(&wal_relative);
    let bytes = match read_authority_file(publication_root, &wal_relative) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::Noop,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::NoWalFile],
                diagnostics: Vec::new(),
            };
        }
        Err(error) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::WalReadFailed],
                diagnostics: vec![format!("read WAL failed: {error}")],
            };
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::WalParseFailed],
                diagnostics: vec![error.to_string()],
            };
        }
    };
    let (records, parse_diagnostics) = match parse_effect_wal_records_for_recovery(&text) {
        Ok(parsed) => parsed,
        Err(error) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::WalParseFailed],
                diagnostics: vec![error.to_string()],
            };
        }
    };
    if let Some((tx_id, reserved)) = reserved_wal_target(&records) {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::ReservedStatePath],
            diagnostics: vec![format!(
                "WAL transaction {tx_id} names EventLog-reserved target: {reserved:?}"
            )],
        };
    }
    let incomplete = incomplete_wal_transactions(&records);
    if incomplete.is_empty() {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::Noop,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::NoRecoveryNeeded],
            diagnostics: parse_diagnostics,
        };
    }
    let mut diagnostics = parse_diagnostics;
    let mut recovered_transactions = Vec::new();
    let mut recovery_ok = true;
    for tx_id in incomplete {
        let before_images: Vec<_> = records
            .iter()
            .filter(|record| record.tx_id == tx_id && record.stage == EffectWalStage::BeforeImage)
            .collect();
        if rollback_wal_before_images_retained(publication_root, &before_images, &mut diagnostics) {
            recovered_transactions.push(tx_id.clone());
            let effect_id = before_images.first().map_or_else(
                || StableId("unknown_effect".to_owned()),
                |record| record.effect_id.clone(),
            );
            if append_effect_wal_record_for_publication(
                publication_root,
                root,
                wal_relative_path,
                EffectWalRecord::stage(&tx_id, effect_id, EffectWalStage::RecoveredRollback),
                WalDurability::SyncOnAppend,
            )
            .is_err()
            {
                recovery_ok = false;
                diagnostics.push(format!(
                    "append recovery marker failed for {}",
                    wal_path.display()
                ));
            }
        } else {
            recovery_ok = false;
        }
    }
    EffectWalRecoveryResult {
        status: if recovery_ok {
            EffectWalRecoveryStatus::Recovered
        } else {
            EffectWalRecoveryStatus::RecoveryFailed
        },
        recovered_transactions,
        reasons: if recovery_ok {
            vec![EffectWalRecoveryReason::IncompleteTransactionRecovered]
        } else {
            vec![EffectWalRecoveryReason::RollbackFailed]
        },
        diagnostics,
    }
}
#[instrument(skip_all, fields(wal_path = %wal_relative_path), level = "info")]
pub fn recover_effect_wal(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
) -> EffectWalRecoveryResult {
    let root = root.as_ref();
    if let Some(reserved) = reserved_state_path(wal_relative_path) {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::ReservedStatePath],
            diagnostics: vec![format!(
                "WAL path is reserved for EventLog TCB: {reserved:?}"
            )],
        };
    }
    if normalized_effect_relative_path(wal_relative_path).is_none() {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::WalReadFailed],
            diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
        };
    }
    let state_root = effect_boundary_state_root(root, wal_relative_path);
    let boundary = match producer_quiescence::admit_effect_producer(&state_root, false) {
        Ok(boundary) => boundary,
        Err(error) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::StoreLockFailed],
                diagnostics: vec![format!("producer boundary failed: {error}")],
            };
        }
    };
    let publication_root =
        match retained_effect_publication_root(&boundary, root, wal_relative_path) {
            Ok(root) => root,
            Err(error) => {
                return EffectWalRecoveryResult {
                    status: EffectWalRecoveryStatus::RecoveryFailed,
                    recovered_transactions: Vec::new(),
                    reasons: vec![EffectWalRecoveryReason::StoreLockFailed],
                    diagnostics: vec![error],
                };
            }
        };
    recover_effect_wal_under_publication_root(root, &publication_root, wal_relative_path)
}

fn parse_effect_wal_records_for_recovery(
    text: &str,
) -> Result<(Vec<EffectWalRecord>, Vec<String>), EffectWalRecoveryParseError> {
    let lines = text.lines().collect::<Vec<_>>();
    let final_line_is_truncated = !text.is_empty() && !text.ends_with('\n');
    let mut records = Vec::new();
    let mut diagnostics = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        match serde_json::from_str::<EffectWalRecord>(line) {
            Ok(record) => records.push(record),
            Err(error) if final_line_is_truncated && index + 1 == lines.len() => {
                diagnostics.push(format!(
                    "ignored truncated final WAL line {}: {error}",
                    index + 1
                ));
            }
            Err(error) => {
                return Err(EffectWalRecoveryParseError::LineParseFailed {
                    line_number: index + 1,
                    source: error.to_string(),
                });
            }
        }
    }

    Ok((records, diagnostics))
}

/// Failures parsing effect WAL records for recovery. Hand-rolled
/// (no anyhow/thiserror); derives Debug, Clone, `PartialEq`, Eq.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectWalRecoveryParseError {
    /// A WAL line could not be parsed as an `EffectWalRecord`.
    LineParseFailed {
        /// 1-based line number of the offending record.
        line_number: usize,
        /// The underlying `serde_json` error, as a lossy String.
        source: String,
    },
}

impl fmt::Display for EffectWalRecoveryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineParseFailed {
                line_number,
                source,
            } => {
                write!(f, "parse WAL line {line_number} failed: {source}")
            }
        }
    }
}

impl std::error::Error for EffectWalRecoveryParseError {}

pub fn rebuild_effect_target_metadata_index_with_lock(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    index_relative_path: &str,
    lock_relative_path: &str,
    recorded_at: Option<&str>,
) -> EffectTargetMetadataIndexRebuildResult {
    rebuild_effect_target_metadata_index_with_lock_with_durability(
        root,
        wal_relative_path,
        index_relative_path,
        lock_relative_path,
        recorded_at,
        WalDurability::default(),
    )
}

/// [`rebuild_effect_target_metadata_index_with_lock`] with an explicit
/// [`WalDurability`] knob. The index file is derived from the WAL, so a
/// crash mid-rebuild loses only rebuild work (the next rebuild call
/// regenerates it from the same WAL). `--no-sync` is therefore semantically
/// safe here. See ADR-0009.
pub fn rebuild_effect_target_metadata_index_with_lock_with_durability(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    index_relative_path: &str,
    lock_relative_path: &str,
    recorded_at: Option<&str>,
    durability: WalDurability,
) -> EffectTargetMetadataIndexRebuildResult {
    let root = root.as_ref();
    match acquire_effect_store_lock(root, lock_relative_path) {
        Ok(lock) => {
            if let Err(error) = validate_effect_lock_scope(root, &lock, lock_relative_path) {
                return EffectTargetMetadataIndexRebuildResult {
                    status: EffectTargetMetadataIndexRebuildStatus::Failed,
                    rebuilt_records: 0,
                    appended_records: 0,
                    records: Vec::new(),
                    reasons: vec![EffectTargetMetadataIndexRebuildReason::StoreLockFailed],
                    diagnostics: vec![error.to_string()],
                };
            }
            rebuild_effect_target_metadata_index_under_publication_root(
                root,
                &lock.root,
                wal_relative_path,
                index_relative_path,
                recorded_at,
                durability,
            )
        }
        Err(error) => EffectTargetMetadataIndexRebuildResult {
            status: EffectTargetMetadataIndexRebuildStatus::Failed,
            rebuilt_records: 0,
            appended_records: 0,
            records: Vec::new(),
            reasons: vec![EffectTargetMetadataIndexRebuildReason::StoreLockFailed],
            diagnostics: vec![format!("effect store lock failed: {error}")],
        },
    }
}

fn rebuild_effect_target_metadata_index_under_publication_root(
    root: &Path,
    publication_root: &retained_dir::RetainedDirectory,
    wal_relative_path: &str,
    index_relative_path: &str,
    recorded_at: Option<&str>,
    durability: WalDurability,
) -> EffectTargetMetadataIndexRebuildResult {
    let Some(wal_relative) = normalized_effect_relative_path(wal_relative_path) else {
        return failed_effect_metadata_rebuild(
            EffectTargetMetadataIndexRebuildReason::WalReadFailed,
            format!("invalid WAL path {wal_relative_path}"),
        );
    };
    let bytes = match read_authority_file(publication_root, &wal_relative) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return EffectTargetMetadataIndexRebuildResult {
                status: EffectTargetMetadataIndexRebuildStatus::Noop,
                rebuilt_records: 0,
                appended_records: 0,
                records: Vec::new(),
                reasons: vec![EffectTargetMetadataIndexRebuildReason::NoWalFile],
                diagnostics: Vec::new(),
            };
        }
        Err(error) => {
            return failed_effect_metadata_rebuild(
                EffectTargetMetadataIndexRebuildReason::WalReadFailed,
                error.to_string(),
            );
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return failed_effect_metadata_rebuild(
                EffectTargetMetadataIndexRebuildReason::WalParseFailed,
                error.to_string(),
            );
        }
    };
    let mut wal_records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        match serde_json::from_str::<EffectWalRecord>(line) {
            Ok(record) => wal_records.push(record),
            Err(error) => {
                return failed_effect_metadata_rebuild(
                    EffectTargetMetadataIndexRebuildReason::WalParseFailed,
                    format!("parse WAL line {} failed: {error}", index + 1),
                );
            }
        }
    }
    let records = effect_target_metadata_records_from_committed_wal(&wal_records, recorded_at);
    if records.is_empty() {
        return EffectTargetMetadataIndexRebuildResult {
            status: EffectTargetMetadataIndexRebuildStatus::Noop,
            rebuilt_records: 0,
            appended_records: 0,
            records,
            reasons: vec![EffectTargetMetadataIndexRebuildReason::NoCommittedMetadataRecords],
            diagnostics: Vec::new(),
        };
    }
    let Some(index_relative) = normalized_effect_relative_path(index_relative_path) else {
        return failed_effect_metadata_rebuild(
            EffectTargetMetadataIndexRebuildReason::MetadataAppendFailed,
            format!("invalid metadata index path {index_relative_path}"),
        );
    };
    let mut content = Vec::new();
    for record in &records {
        match serde_json::to_vec(record) {
            Ok(mut line) => {
                content.append(&mut line);
                content.push(b'\n');
            }
            Err(error) => {
                return failed_effect_metadata_rebuild(
                    EffectTargetMetadataIndexRebuildReason::MetadataAppendFailed,
                    error.to_string(),
                );
            }
        }
    }
    let append = publication_root
        .open_read_write_create(&index_relative)
        .and_then(|mut file| {
            file.seek(SeekFrom::End(0))?;
            file.write_all(&content)?;
            match durability {
                WalDurability::SyncOnAppend => file.sync_data(),
                WalDurability::NoSync => Ok(()),
            }
        });
    if let Err(error) = append {
        return failed_effect_metadata_rebuild(
            EffectTargetMetadataIndexRebuildReason::MetadataAppendFailed,
            format!(
                "append metadata index {} failed: {error}",
                root.join(index_relative).display()
            ),
        );
    }
    EffectTargetMetadataIndexRebuildResult {
        status: EffectTargetMetadataIndexRebuildStatus::Rebuilt,
        rebuilt_records: records.len(),
        appended_records: records.len(),
        records,
        reasons: vec![EffectTargetMetadataIndexRebuildReason::MetadataRebuilt],
        diagnostics: Vec::new(),
    }
}

fn failed_effect_metadata_rebuild(
    reason: EffectTargetMetadataIndexRebuildReason,
    diagnostic: String,
) -> EffectTargetMetadataIndexRebuildResult {
    EffectTargetMetadataIndexRebuildResult {
        status: EffectTargetMetadataIndexRebuildStatus::Failed,
        rebuilt_records: 0,
        appended_records: 0,
        records: Vec::new(),
        reasons: vec![reason],
        diagnostics: vec![diagnostic],
    }
}
pub fn rebuild_effect_target_metadata_index(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    index_relative_path: &str,
    recorded_at: Option<&str>,
) -> EffectTargetMetadataIndexRebuildResult {
    rebuild_effect_target_metadata_index_with_durability(
        root,
        wal_relative_path,
        index_relative_path,
        recorded_at,
        WalDurability::default(),
    )
}

/// [`rebuild_effect_target_metadata_index`] with an explicit [`WalDurability`] knob.
pub fn rebuild_effect_target_metadata_index_with_durability(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    index_relative_path: &str,
    recorded_at: Option<&str>,
    durability: WalDurability,
) -> EffectTargetMetadataIndexRebuildResult {
    let root = root.as_ref();
    if normalized_effect_relative_path(wal_relative_path).is_none() {
        return failed_effect_metadata_rebuild(
            EffectTargetMetadataIndexRebuildReason::WalReadFailed,
            format!("invalid WAL path {wal_relative_path}"),
        );
    }
    let state_root = effect_boundary_state_root(root, wal_relative_path);
    let boundary = match producer_quiescence::admit_effect_producer(&state_root, false) {
        Ok(boundary) => boundary,
        Err(error) => {
            return failed_effect_metadata_rebuild(
                EffectTargetMetadataIndexRebuildReason::StoreLockFailed,
                format!("producer boundary failed: {error}"),
            );
        }
    };
    let publication_root =
        match retained_effect_publication_root(&boundary, root, wal_relative_path) {
            Ok(root) => root,
            Err(error) => {
                return failed_effect_metadata_rebuild(
                    EffectTargetMetadataIndexRebuildReason::StoreLockFailed,
                    error,
                );
            }
        };
    rebuild_effect_target_metadata_index_under_publication_root(
        root,
        &publication_root,
        wal_relative_path,
        index_relative_path,
        recorded_at,
        durability,
    )
}

pub fn query_effect_target_metadata_index(
    root: impl AsRef<Path>,
    index_relative_path: &str,
    query: &EffectTargetMetadataIndexQuery,
) -> EffectTargetMetadataIndexQueryResult {
    let root = root.as_ref();
    let Some(index_relative) = normalized_effect_relative_path(index_relative_path) else {
        return EffectTargetMetadataIndexQueryResult {
            status: EffectTargetMetadataIndexQueryStatus::Failed,
            index_relative_path: index_relative_path.to_string(),
            consumer_use: query.consumer_use,
            scanned_records: 0,
            matched_records: 0,
            returned_records: 0,
            latest_per_target: query.latest_per_target,
            records: Vec::new(),
            authority_boundary: effect_metadata_authority_boundary(),
            reasons: vec![EffectTargetMetadataIndexQueryReason::IndexReadFailed],
            diagnostics: vec![format!("invalid index path {index_relative_path}")],
        };
    };
    let publication_root = match retained_dir::RetainedDirectory::open_root(root) {
        Ok(root) => root,
        Err(error) => {
            return EffectTargetMetadataIndexQueryResult {
                status: EffectTargetMetadataIndexQueryStatus::Failed,
                index_relative_path: index_relative_path.to_string(),
                consumer_use: query.consumer_use,
                scanned_records: 0,
                matched_records: 0,
                returned_records: 0,
                latest_per_target: query.latest_per_target,
                records: Vec::new(),
                authority_boundary: effect_metadata_authority_boundary(),
                reasons: vec![EffectTargetMetadataIndexQueryReason::IndexReadFailed],
                diagnostics: vec![format!("open metadata authority root failed: {error}")],
            };
        }
    };
    let bytes = match read_authority_file(&publication_root, &index_relative) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return EffectTargetMetadataIndexQueryResult {
                status: EffectTargetMetadataIndexQueryStatus::Noop,
                index_relative_path: index_relative_path.to_string(),
                consumer_use: query.consumer_use,
                scanned_records: 0,
                matched_records: 0,
                returned_records: 0,
                latest_per_target: query.latest_per_target,
                records: Vec::new(),
                authority_boundary: effect_metadata_authority_boundary(),
                reasons: vec![EffectTargetMetadataIndexQueryReason::NoIndexFile],
                diagnostics: Vec::new(),
            };
        }
        Err(error) => {
            return EffectTargetMetadataIndexQueryResult {
                status: EffectTargetMetadataIndexQueryStatus::Failed,
                index_relative_path: index_relative_path.to_string(),
                consumer_use: query.consumer_use,
                scanned_records: 0,
                matched_records: 0,
                returned_records: 0,
                latest_per_target: query.latest_per_target,
                records: Vec::new(),
                authority_boundary: effect_metadata_authority_boundary(),
                reasons: vec![EffectTargetMetadataIndexQueryReason::IndexReadFailed],
                diagnostics: vec![format!("read metadata index failed: {error}")],
            };
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return EffectTargetMetadataIndexQueryResult {
                status: EffectTargetMetadataIndexQueryStatus::Failed,
                index_relative_path: index_relative_path.to_string(),
                consumer_use: query.consumer_use,
                scanned_records: 0,
                matched_records: 0,
                returned_records: 0,
                latest_per_target: query.latest_per_target,
                records: Vec::new(),
                authority_boundary: effect_metadata_authority_boundary(),
                reasons: vec![EffectTargetMetadataIndexQueryReason::IndexReadFailed],
                diagnostics: vec![format!("read metadata index failed: {error}")],
            };
        }
    };

    let mut scanned_records = 0usize;
    let mut matched_records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let record = match serde_json::from_str::<EffectTargetMetadataRecord>(line) {
            Ok(record) => record,
            Err(error) => {
                return EffectTargetMetadataIndexQueryResult {
                    status: EffectTargetMetadataIndexQueryStatus::Failed,
                    index_relative_path: index_relative_path.to_string(),
                    consumer_use: query.consumer_use,
                    scanned_records,
                    matched_records: matched_records.len(),
                    returned_records: 0,
                    latest_per_target: query.latest_per_target,
                    records: Vec::new(),
                    authority_boundary: effect_metadata_authority_boundary(),
                    reasons: vec![EffectTargetMetadataIndexQueryReason::IndexParseFailed],
                    diagnostics: vec![format!(
                        "parse metadata index line {} failed: {error}",
                        index + 1
                    )],
                };
            }
        };
        scanned_records += 1;
        if effect_target_metadata_query_matches(&record, query) {
            matched_records.push(record);
        }
    }

    let records = if query.latest_per_target {
        latest_effect_target_metadata_records(&matched_records)
    } else {
        matched_records.clone()
    };
    let reason = if matched_records.is_empty() {
        EffectTargetMetadataIndexQueryReason::NoMatches
    } else {
        EffectTargetMetadataIndexQueryReason::QueryMatched
    };
    EffectTargetMetadataIndexQueryResult {
        status: EffectTargetMetadataIndexQueryStatus::Queried,
        index_relative_path: index_relative_path.to_string(),
        consumer_use: query.consumer_use,
        scanned_records,
        matched_records: matched_records.len(),
        returned_records: records.len(),
        latest_per_target: query.latest_per_target,
        records,
        authority_boundary: effect_metadata_authority_boundary(),
        reasons: vec![reason],
        diagnostics: Vec::new(),
    }
}

#[must_use]
pub fn build_effect_metadata_context(
    query_result: &EffectTargetMetadataIndexQueryResult,
    options: &EffectMetadataContextBuildOptions,
) -> EffectMetadataContextBuildResult {
    if query_result.records.is_empty() {
        return EffectMetadataContextBuildResult {
            status: EffectMetadataContextBuildStatus::Empty,
            source_status: query_result.status,
            source_consumer_use: query_result.consumer_use,
            total_groups: 0,
            returned_groups: 0,
            omitted_groups: 0,
            included_records: 0,
            groups: Vec::new(),
            adapter_presentation: effect_metadata_adapter_presentation(options),
            authority_boundary: effect_metadata_authority_boundary(),
            reasons: vec![EffectMetadataContextBuildReason::NoQueryRecords],
            diagnostics: Vec::new(),
        };
    }

    let mut grouped = BTreeMap::<String, (usize, EffectTargetMetadataRecord)>::new();
    for record in &query_result.records {
        let entry = grouped
            .entry(effect_target_metadata_latest_key(record))
            .or_insert_with(|| (0, record.clone()));
        entry.0 += 1;
        entry.1 = record.clone();
    }

    let total_groups = grouped.len();
    let max_groups = options.max_groups.max(1);
    let omitted_groups = total_groups.saturating_sub(max_groups);
    let groups = grouped
        .into_values()
        .take(max_groups)
        .map(|(record_count, latest)| EffectMetadataContextGroup {
            target_kind: latest.target_kind,
            logical_ref: latest.logical_ref,
            record_count,
            latest_physical_ref: latest.physical_ref,
            latest_effect_id: latest.effect_id,
            latest_operation_id: latest.operation_id,
            latest_recorded_at: latest.recorded_at,
            latest_access_mode: latest.access_mode,
            latest_content_hash: latest.content_hash,
            latest_byte_len: latest.byte_len,
            latest_actor_agent_id: latest.actor_agent_id,
            latest_actor_role: latest.actor_role,
            destructive: latest.destructive,
            redaction_hint: latest.redaction_hint,
        })
        .collect::<Vec<_>>();
    let mut reasons = vec![EffectMetadataContextBuildReason::ContextBuilt];
    if omitted_groups > 0 {
        reasons.push(EffectMetadataContextBuildReason::GroupsOmittedByLimit);
    }

    EffectMetadataContextBuildResult {
        status: EffectMetadataContextBuildStatus::Built,
        source_status: query_result.status,
        source_consumer_use: query_result.consumer_use,
        total_groups,
        returned_groups: groups.len(),
        omitted_groups,
        included_records: groups.iter().map(|group| group.record_count).sum(),
        groups,
        adapter_presentation: effect_metadata_adapter_presentation(options),
        authority_boundary: effect_metadata_authority_boundary(),
        reasons,
        diagnostics: Vec::new(),
    }
}

pub fn compact_effect_wal_with_lock(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    lock_relative_path: &str,
) -> EffectWalCompactionResult {
    let root = root.as_ref();
    match acquire_effect_store_lock(root, lock_relative_path) {
        Ok(lock) => {
            if let Err(error) = validate_effect_lock_scope(root, &lock, lock_relative_path) {
                return failed_effect_wal_compaction(
                    EffectWalCompactionReason::StoreLockFailed,
                    error.to_string(),
                );
            }
            compact_effect_wal_under_publication_root(root, &lock.root, wal_relative_path)
        }
        Err(error) => failed_effect_wal_compaction(
            EffectWalCompactionReason::StoreLockFailed,
            format!("effect store lock failed: {error}"),
        ),
    }
}

fn compact_effect_wal_under_publication_root(
    root: &Path,
    publication_root: &retained_dir::RetainedDirectory,
    wal_relative_path: &str,
) -> EffectWalCompactionResult {
    let Some(wal_relative) = normalized_effect_relative_path(wal_relative_path) else {
        return failed_effect_wal_compaction(
            EffectWalCompactionReason::WalReadFailed,
            format!("invalid WAL path {wal_relative_path}"),
        );
    };
    let wal_path = root.join(&wal_relative);
    let bytes = match read_authority_file(publication_root, &wal_relative) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return EffectWalCompactionResult {
                status: EffectWalCompactionStatus::Noop,
                retained_records: 0,
                dropped_records: 0,
                incomplete_transactions: Vec::new(),
                reasons: vec![EffectWalCompactionReason::NoWalFile],
                diagnostics: Vec::new(),
            };
        }
        Err(error) => {
            return failed_effect_wal_compaction(
                EffectWalCompactionReason::WalReadFailed,
                error.to_string(),
            );
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return failed_effect_wal_compaction(
                EffectWalCompactionReason::WalParseFailed,
                error.to_string(),
            );
        }
    };
    let records = match text
        .lines()
        .map(serde_json::from_str::<EffectWalRecord>)
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(records) => records,
        Err(error) => {
            return failed_effect_wal_compaction(
                EffectWalCompactionReason::WalParseFailed,
                error.to_string(),
            );
        }
    };
    let incomplete_transactions = incomplete_wal_transactions(&records);
    let provenance_transactions = provenance_bound_transaction_ids(&records);
    let retained = records
        .iter()
        .filter(|record| {
            incomplete_transactions.contains(&record.tx_id)
                || provenance_transactions.contains(&record.tx_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let dropped_records = records.len().saturating_sub(retained.len());
    if dropped_records == 0 {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Noop,
            retained_records: records.len(),
            dropped_records: 0,
            incomplete_transactions,
            reasons: if provenance_transactions.is_empty() {
                vec![EffectWalCompactionReason::NoClosedRecords]
            } else {
                vec![
                    EffectWalCompactionReason::NoClosedRecords,
                    EffectWalCompactionReason::ProvenanceRecordsRetained,
                ]
            },
            diagnostics: Vec::new(),
        };
    }
    let mut content = Vec::new();
    for record in &retained {
        match serde_json::to_vec(record) {
            Ok(mut line) => {
                content.append(&mut line);
                content.push(b'\n');
            }
            Err(error) => {
                return failed_effect_wal_compaction(
                    EffectWalCompactionReason::WalWriteFailed,
                    error.to_string(),
                );
            }
        }
    }
    let Some(file_name) = wal_relative.file_name().and_then(|name| name.to_str()) else {
        return failed_effect_wal_compaction(
            EffectWalCompactionReason::WalWriteFailed,
            "WAL path has no file name".to_owned(),
        );
    };
    let parent = wal_relative.parent().unwrap_or_else(|| Path::new("."));
    let manifest_relative = parent.join(format!(".{file_name}.compaction-manifest.json"));
    let manifest = |status: &str| EffectWalCompactionManifest {
        schema_version: "0.1".to_owned(),
        wal_relative_path: wal_relative_path.to_owned(),
        status: status.to_owned(),
        retained_records: retained.len(),
        dropped_records,
        incomplete_transactions: incomplete_transactions.clone(),
    };
    let write_manifest = |status: &str| -> io::Result<()> {
        let bytes = serde_json::to_vec(&manifest(status))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        let expected = read_optional_authority_file(publication_root, &manifest_relative)?;
        atomic_replace_file_retained(
            publication_root,
            &manifest_relative,
            &bytes,
            expected.as_deref(),
        )
    };
    if let Err(error) = write_manifest("begin") {
        return failed_effect_wal_compaction(
            EffectWalCompactionReason::WalWriteFailed,
            format!("write WAL compaction manifest failed: {error}"),
        );
    }
    if let Err(error) = atomic_replace_file_retained(
        publication_root,
        &wal_relative,
        &content,
        Some(text.as_bytes()),
    ) {
        return failed_effect_wal_compaction(
            EffectWalCompactionReason::WalWriteFailed,
            format!("write compacted WAL {} failed: {error}", wal_path.display()),
        );
    }
    if let Err(error) = write_manifest("complete") {
        return failed_effect_wal_compaction(
            EffectWalCompactionReason::WalWriteFailed,
            format!("complete WAL compaction manifest failed: {error}"),
        );
    }
    let mut reasons = vec![EffectWalCompactionReason::ClosedRecordsDropped];
    if !provenance_transactions.is_empty() {
        reasons.push(EffectWalCompactionReason::ProvenanceRecordsRetained);
    }
    EffectWalCompactionResult {
        status: EffectWalCompactionStatus::Compacted,
        retained_records: retained.len(),
        dropped_records,
        incomplete_transactions,
        reasons,
        diagnostics: Vec::new(),
    }
}

fn failed_effect_wal_compaction(
    reason: EffectWalCompactionReason,
    diagnostic: String,
) -> EffectWalCompactionResult {
    EffectWalCompactionResult {
        status: EffectWalCompactionStatus::Failed,
        retained_records: 0,
        dropped_records: 0,
        incomplete_transactions: Vec::new(),
        reasons: vec![reason],
        diagnostics: vec![diagnostic],
    }
}
pub fn compact_effect_wal(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
) -> EffectWalCompactionResult {
    let root = root.as_ref();
    if normalized_effect_relative_path(wal_relative_path).is_none() {
        return failed_effect_wal_compaction(
            EffectWalCompactionReason::WalReadFailed,
            format!("invalid WAL path {wal_relative_path}"),
        );
    }
    let state_root = effect_boundary_state_root(root, wal_relative_path);
    let boundary = match producer_quiescence::admit_effect_producer(&state_root, false) {
        Ok(boundary) => boundary,
        Err(error) => {
            return failed_effect_wal_compaction(
                EffectWalCompactionReason::StoreLockFailed,
                format!("producer boundary failed: {error}"),
            );
        }
    };
    let publication_root =
        match retained_effect_publication_root(&boundary, root, wal_relative_path) {
            Ok(root) => root,
            Err(error) => {
                return failed_effect_wal_compaction(
                    EffectWalCompactionReason::StoreLockFailed,
                    error,
                );
            }
        };
    compact_effect_wal_under_publication_root(root, &publication_root, wal_relative_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceIndexBuildError {
    PathOutsideRoot { root: PathBuf, path: PathBuf },
    ReadDir { path: PathBuf, source: String },
    ReadFile { path: PathBuf, source: String },
    ParseYaml { path: PathBuf, source: String },
}

impl fmt::Display for ReferenceIndexBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathOutsideRoot { root, path } => {
                write!(
                    formatter,
                    "path {} is not under repo root {}",
                    path.display(),
                    root.display()
                )
            }
            Self::ReadDir { path, source } => {
                write!(formatter, "read dir {}: {source}", path.display())
            }
            Self::ReadFile { path, source } => {
                write!(formatter, "read file {}: {source}", path.display())
            }
            Self::ParseYaml { path, source } => {
                write!(formatter, "parse yaml {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for ReferenceIndexBuildError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendJsonLineError {
    InvalidRelativePath {
        path: String,
    },
    ReservedStatePath {
        path: String,
        reserved: ReservedStatePath,
    },
    CreateDir {
        path: PathBuf,
        source: String,
    },
    OpenFile {
        path: PathBuf,
        source: String,
    },
    Serialize {
        path: PathBuf,
        source: String,
    },
    Write {
        path: PathBuf,
        source: String,
    },
    Lock {
        path: String,
        source: String,
    },
}

impl fmt::Display for AppendJsonLineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath { path } => {
                write!(formatter, "invalid repo-relative append path {path}")
            }
            Self::ReservedStatePath { path, reserved } => {
                write!(
                    formatter,
                    "append path {path} is reserved for EventLog TCB: {reserved:?}"
                )
            }
            Self::CreateDir { path, source } => {
                write!(formatter, "create dir {}: {source}", path.display())
            }
            Self::OpenFile { path, source } => {
                write!(formatter, "open append file {}: {source}", path.display())
            }
            Self::Serialize { path, source } => {
                write!(
                    formatter,
                    "serialize append record {}: {source}",
                    path.display()
                )
            }
            Self::Write { path, source } => {
                write!(
                    formatter,
                    "write append record {}: {source}",
                    path.display()
                )
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock append record {path}: {source}")
            }
        }
    }
}

impl std::error::Error for AppendJsonLineError {}

impl fmt::Display for EffectStoreLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath { path } => {
                write!(formatter, "invalid repo-relative lock path {path}")
            }
            Self::ReservedStatePath { path, reserved } => {
                write!(
                    formatter,
                    "lock path {path} is reserved for EventLog TCB: {reserved:?}"
                )
            }
            Self::CreateDir { path, source } => {
                write!(formatter, "create lock dir {}: {source}", path.display())
            }
            Self::OpenFile { path, source } => {
                write!(formatter, "open lock file {}: {source}", path.display())
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock file {}: {source}", path.display())
            }
            Self::ProducerBoundary { source } => {
                write!(formatter, "producer boundary: {source}")
            }
            Self::WouldBlock { path } => {
                write!(formatter, "lock file {} would block", path.display())
            }
        }
    }
}

impl std::error::Error for EffectStoreLockError {}

fn add_contract_definitions(
    index: &mut ReferenceIndex,
    root: &Path,
    known_embedded: &std::collections::HashSet<String>,
) {
    for reference in CONTRACT_DEFINITIONS {
        insert_existing(
            index,
            root,
            reference,
            ReferenceKind::ContractDefinition,
            known_embedded,
        );
    }
}

fn add_policy_files(
    index: &mut ReferenceIndex,
    root: &Path,
    known_embedded: &std::collections::HashSet<String>,
) -> Result<(), ReferenceIndexBuildError> {
    add_yaml_files_in_dir(index, root, "contracts/policies", ReferenceKind::Policy)?;
    insert_existing(
        index,
        root,
        "contracts/operations/operation-reference-policy-v0.yaml",
        ReferenceKind::Policy,
        known_embedded,
    );
    Ok(())
}

fn add_operation_fixtures(
    index: &mut ReferenceIndex,
    root: &Path,
) -> Result<(), ReferenceIndexBuildError> {
    add_yaml_files_in_dir(
        index,
        root,
        "docs/fixtures/operation-contract-v0",
        ReferenceKind::OperationFixture,
    )
}

fn add_contract_instances(
    index: &mut ReferenceIndex,
    root: &Path,
    _known_embedded: &std::collections::HashSet<String>,
) -> Result<(), ReferenceIndexBuildError> {
    add_instance_dir(
        index,
        root,
        "contracts/claims",
        "claim-contract-v0.yaml",
        ReferenceKind::ClaimContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/completion",
        "completion-contract-v0.yaml",
        ReferenceKind::CompletionContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/gates",
        "gate-contract-v0.yaml",
        ReferenceKind::GateContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/requests",
        "request-contract-v0.yaml",
        ReferenceKind::RequestContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        ReferenceKind::ToolEffectContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        ReferenceKind::DecisionCloseContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        ReferenceKind::HealthRecoveryContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        ReferenceKind::CoordinationEvalContract,
    )?;
    add_instance_dir(
        index,
        root,
        "contracts/inventory",
        "contract-family-inventory-v0.yaml",
        ReferenceKind::InventoryContract,
    )
}

fn add_command_contracts(
    index: &mut ReferenceIndex,
    root: &Path,
) -> Result<(), ReferenceIndexBuildError> {
    let dir = root.join("contracts/commands");
    for path in yaml_files(&dir)? {
        if file_name_is(&path, "command-contract-v0.yaml") {
            continue;
        }
        index.insert(repo_relative(root, &path)?, ReferenceKind::CommandContract);
        let value = read_yaml_value(&path)?;
        if let Some(id) = nested_str(&value, &["command_contract", "id"]) {
            index.insert(id, ReferenceKind::CommandContract);
        }
    }
    Ok(())
}

fn add_runtime_contracts(
    index: &mut ReferenceIndex,
    root: &Path,
    _known_embedded: &std::collections::HashSet<String>,
) -> Result<(), ReferenceIndexBuildError> {
    let dir = root.join("contracts/runtimes");
    for path in yaml_files(&dir)? {
        if file_name_is(&path, "runtime-handoff-contract-v0.yaml") {
            continue;
        }
        let value = read_yaml_value(&path)?;
        let kind = if value.get("runtime_handoff_contract").is_some() {
            Some(ReferenceKind::RuntimeHandoffContract)
        } else if value.get("runtime_registry_entry").is_some() {
            Some(ReferenceKind::RuntimeRegistryEntry)
        } else if value.get("runtime_capability").is_some() {
            Some(ReferenceKind::RuntimeCapability)
        } else {
            None
        };
        if let Some(kind) = kind {
            index.insert(repo_relative(root, &path)?, kind);
        }
    }
    Ok(())
}

fn add_runtime_state_refs(
    index: &mut ReferenceIndex,
    root: &Path,
    options: &ReferenceIndexOptions,
    known_embedded: &std::collections::HashSet<String>,
) {
    insert_existing(
        index,
        root,
        ".forge-method/ledger.ndjson",
        ReferenceKind::Ledger,
        known_embedded,
    );
    if options.include_standard_runtime_projections {
        index.insert(
            ".forge-method/agents/registry.yaml",
            ReferenceKind::RuntimeRegistryProjection,
        );
    }
}

fn add_instance_dir(
    index: &mut ReferenceIndex,
    root: &Path,
    relative_dir: &str,
    definition_file: &str,
    kind: ReferenceKind,
) -> Result<(), ReferenceIndexBuildError> {
    let dir = root.join(relative_dir);
    for path in yaml_files(&dir)? {
        if !file_name_is(&path, definition_file) {
            index.insert(repo_relative(root, &path)?, kind);
        }
    }
    Ok(())
}

fn add_yaml_files_in_dir(
    index: &mut ReferenceIndex,
    root: &Path,
    relative_dir: &str,
    kind: ReferenceKind,
) -> Result<(), ReferenceIndexBuildError> {
    let dir = root.join(relative_dir);
    for path in yaml_files(&dir)? {
        index.insert(repo_relative(root, &path)?, kind);
    }
    Ok(())
}

fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>, ReferenceIndexBuildError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(dir).map_err(|source| ReferenceIndexBuildError::ReadDir {
        path: dir.to_path_buf(),
        source: source.to_string(),
    })?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ReferenceIndexBuildError::ReadDir {
            path: dir.to_path_buf(),
            source: source.to_string(),
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn repo_relative_for_diagnostic(
    root: &Path,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagnosticCode,
) -> Option<String> {
    match repo_relative(root, path) {
        Ok(relative_path) => Some(relative_path),
        Err(source) => {
            diagnostics.push(Diagnostic::error(
                code,
                diagnostic_path(path),
                source.to_string(),
            ));
            None
        }
    }
}

fn collect_yaml_documents_recursive(
    root: &Path,
    dir: &Path,
    collection: &mut YamlDocumentCollection,
) {
    if !dir.exists() {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(source) => {
            if let Some(relative_path) = repo_relative_for_diagnostic(
                root,
                dir,
                &mut collection.diagnostics,
                DiagnosticCode::YamlReadFailed,
            ) {
                collection.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::YamlReadFailed,
                    relative_path,
                    source.to_string(),
                ));
            }
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                if let Some(relative_path) = repo_relative_for_diagnostic(
                    root,
                    dir,
                    &mut collection.diagnostics,
                    DiagnosticCode::YamlReadFailed,
                ) {
                    collection.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::YamlReadFailed,
                        relative_path,
                        source.to_string(),
                    ));
                }
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_documents_recursive(root, &path, collection);
        } else if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            match read_yaml_value(&path) {
                Ok(value) => {
                    if let Some(relative_path) = repo_relative_for_diagnostic(
                        root,
                        &path,
                        &mut collection.diagnostics,
                        DiagnosticCode::YamlReadFailed,
                    ) {
                        collection.documents.push(ParsedYamlDocument {
                            path: relative_path,
                            value,
                        });
                    }
                }
                Err(source) => {
                    if let Some(relative_path) = repo_relative_for_diagnostic(
                        root,
                        &path,
                        &mut collection.diagnostics,
                        DiagnosticCode::YamlParseFailed,
                    ) {
                        collection.diagnostics.push(Diagnostic::error(
                            DiagnosticCode::YamlParseFailed,
                            relative_path,
                            source.to_string(),
                        ));
                    }
                }
            }
        }
    }
}

fn collect_known_paths_recursive(
    root: &Path,
    path: &Path,
    collection: &mut KnownRepoPathsCollection,
) {
    if !path.exists() {
        return;
    }
    let Some(relative_path) = repo_relative_for_diagnostic(
        root,
        path,
        &mut collection.diagnostics,
        DiagnosticCode::YamlReadFailed,
    ) else {
        return;
    };
    collection.paths.insert(relative_path);
    if !path.is_dir() {
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(source) => {
            if let Some(relative_path) = repo_relative_for_diagnostic(
                root,
                path,
                &mut collection.diagnostics,
                DiagnosticCode::YamlReadFailed,
            ) {
                collection.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::YamlReadFailed,
                    relative_path,
                    source.to_string(),
                ));
            }
            return;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => collect_known_paths_recursive(root, &entry.path(), collection),
            Err(source) => {
                if let Some(relative_path) = repo_relative_for_diagnostic(
                    root,
                    path,
                    &mut collection.diagnostics,
                    DiagnosticCode::YamlReadFailed,
                ) {
                    collection.diagnostics.push(Diagnostic::error(
                        DiagnosticCode::YamlReadFailed,
                        relative_path,
                        source.to_string(),
                    ));
                }
            }
        }
    }
}

fn read_yaml_value(path: &Path) -> Result<Value, ReferenceIndexBuildError> {
    let text = fs::read_to_string(path).map_err(|source| ReferenceIndexBuildError::ReadFile {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    yaml_serde::from_str(&text).map_err(|source| ReferenceIndexBuildError::ParseYaml {
        path: path.to_path_buf(),
        source: source.to_string(),
    })
}

fn insert_existing(
    index: &mut ReferenceIndex,
    root: &Path,
    reference: &str,
    kind: ReferenceKind,
    known_embedded: &std::collections::HashSet<String>,
) {
    // Register if the file is present on disk OR if it is a canonically
    // known embedded contract (a consumer repo may ship no contracts/ tree
    // but still reference the shared definitions served from the binary).
    if root.join(reference).exists() || known_embedded.contains(reference) {
        index.insert(reference, kind);
    }
}

fn resolve_safe_repo_relative(
    root: &Path,
    relative_path: &str,
) -> Result<PathBuf, AppendJsonLineError> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(forbidden_relative_component)
    {
        return Err(AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_string(),
        });
    }

    let canonical_root =
        canonicalize_maybe_missing(root).map_err(|_| AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_string(),
        })?;
    let components = path_components(path);
    if components.is_empty() {
        return Err(AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_string(),
        });
    }

    let mut resolved = canonical_root.clone();
    for (index, component) in components.iter().enumerate() {
        let candidate = resolved.join(component);
        if candidate.exists() {
            let canonical_candidate = fs::canonicalize(&candidate).map_err(|_| {
                AppendJsonLineError::InvalidRelativePath {
                    path: relative_path.to_string(),
                }
            })?;
            if !canonical_candidate.starts_with(&canonical_root) {
                return Err(AppendJsonLineError::InvalidRelativePath {
                    path: relative_path.to_string(),
                });
            }
            resolved = canonical_candidate;
        } else {
            resolved = candidate;
            for remaining in components.iter().skip(index + 1) {
                resolved.push(remaining);
            }
            break;
        }
    }

    if !resolved_parent_stays_within_root(&canonical_root, &resolved) {
        return Err(AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_string(),
        });
    }
    Ok(resolved)
}

fn canonicalize_maybe_missing(path: &Path) -> io::Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path);
    }

    let mut missing = Vec::new();
    let mut ancestor = path;
    while !ancestor.exists() {
        let Some(file_name) = ancestor.file_name() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no existing ancestor",
            ));
        };
        missing.push(file_name.to_os_string());
        let Some(parent) = ancestor.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no existing ancestor",
            ));
        };
        ancestor = parent;
    }

    let mut resolved = fs::canonicalize(ancestor)?;
    for component in missing.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn path_components(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_os_string()),
            _ => None,
        })
        .collect()
}

fn resolved_parent_stays_within_root(canonical_root: &Path, resolved: &Path) -> bool {
    let Some(parent) = resolved.parent() else {
        return false;
    };
    if parent.exists() {
        return fs::canonicalize(parent)
            .is_ok_and(|canonical_parent| canonical_parent.starts_with(canonical_root));
    }

    let mut ancestor = parent;
    while !ancestor.exists() {
        let Some(next) = ancestor.parent() else {
            return false;
        };
        ancestor = next;
    }
    fs::canonicalize(ancestor).is_ok_and(|canonical_ancestor| {
        canonical_ancestor.starts_with(canonical_root) || resolved.starts_with(canonical_root)
    })
}

fn ensure_resolved_parent_within_root(root: &Path, target: &Path) -> io::Result<()> {
    let canonical_root = fs::canonicalize(root)?;
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let canonical_parent = fs::canonicalize(parent)?;
    if canonical_parent.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "repo-relative target parent escapes root",
        ))
    }
}

fn ensure_target_chain_within_root(root: &Path, target: &Path) -> io::Result<()> {
    let canonical_root = canonicalize_maybe_missing(root)?;
    if resolved_parent_stays_within_root(&canonical_root, target) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "repo-relative target parent escapes root",
        ))
    }
}

fn append_json_line_lock_relative_path(relative_path: &str) -> String {
    let digest = Sha256::digest(relative_path.as_bytes());
    format!(".forge-method/locks/append-json-line/{digest:x}.lock")
}

fn state_append_json_line_lock_relative_path(relative_path: &str) -> String {
    let digest = Sha256::digest(relative_path.as_bytes());
    format!("locks/append-json-line/{digest:x}.lock")
}

fn resolve_effect_target(
    root: &Path,
    target_kind: EffectTargetKind,
    reference: &str,
) -> Result<(PathBuf, String), AppendJsonLineError> {
    let physical_ref = effect_target_physical_reference(target_kind, reference)?;
    match reject_reserved_state_mutation(&physical_ref) {
        Err(ReservedStateMutationError::Reserved(reserved)) => {
            return Err(AppendJsonLineError::ReservedStatePath {
                path: physical_ref,
                reserved,
            });
        }
        Err(ReservedStateMutationError::Invalid) => {
            return Err(AppendJsonLineError::InvalidRelativePath { path: physical_ref });
        }
        Ok(_) => {}
    }
    let path = resolve_safe_repo_relative(root, &physical_ref)?;
    Ok((path, physical_ref))
}

fn effect_target_physical_reference(
    target_kind: EffectTargetKind,
    reference: &str,
) -> Result<String, AppendJsonLineError> {
    match target_kind {
        EffectTargetKind::FilePath => Ok(reference.to_string()),
        EffectTargetKind::ArtifactId => project_logical_target(
            reference,
            &[".forge-method/artifacts/"],
            &[],
            ".forge-method/artifacts",
            ".yaml",
        ),
        EffectTargetKind::EvidenceId => project_logical_target(
            reference,
            &[".forge-method/evidence/", ".forge-method/snapshots/"],
            &[],
            ".forge-method/evidence",
            ".json",
        ),
        EffectTargetKind::LedgerStream => project_logical_target(
            reference,
            &[".forge-method/ledger/"],
            &[".forge-method/ledger.ndjson"],
            ".forge-method/ledger",
            ".ndjson",
        ),
        EffectTargetKind::RequestStream => project_logical_target(
            reference,
            &[".forge-method/requests/"],
            &[".forge-method/requests.ndjson"],
            ".forge-method/requests",
            ".ndjson",
        ),
        EffectTargetKind::Glob | EffectTargetKind::StateKey | EffectTargetKind::CompletionId => {
            Err(AppendJsonLineError::InvalidRelativePath {
                path: reference.to_string(),
            })
        }
    }
}

fn project_logical_target(
    reference: &str,
    allowed_prefixes: &[&str],
    allowed_exact: &[&str],
    base_dir: &str,
    extension: &str,
) -> Result<String, AppendJsonLineError> {
    if reference_looks_path_like(reference) {
        if allowed_exact.contains(&reference)
            || allowed_prefixes
                .iter()
                .any(|prefix| reference.starts_with(prefix))
        {
            Ok(reference.to_string())
        } else {
            Err(AppendJsonLineError::InvalidRelativePath {
                path: reference.to_string(),
            })
        }
    } else {
        let Some(safe_id) = safe_logical_id(reference) else {
            return Err(AppendJsonLineError::InvalidRelativePath {
                path: reference.to_string(),
            });
        };
        let file_name = if safe_id.ends_with(extension) {
            safe_id
        } else {
            format!("{safe_id}{extension}")
        };
        Ok(format!("{base_dir}/{file_name}"))
    }
}

fn reference_looks_path_like(reference: &str) -> bool {
    reference.contains('/') || reference.contains('\\')
}

fn safe_logical_id(reference: &str) -> Option<String> {
    let sanitized = reference
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('.').to_string();
    if trimmed.is_empty() || !trimmed.chars().any(|ch| ch.is_ascii_alphanumeric()) {
        None
    } else {
        Some(trimmed)
    }
}

fn forbidden_relative_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir
    )
}

#[derive(Debug, Clone)]
struct PreparedWrite {
    reference: String,
    physical_reference: String,
    target: PathBuf,
    target_kind: EffectTargetKind,
    access_mode: PreparedAccessMode,
    destructive: bool,
    expected_hash: Option<String>,
    payload_content: Option<Vec<u8>>,
    content: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreparedAccessMode {
    Create,
    Write,
    Append,
    Delete,
}

impl PreparedAccessMode {
    fn from_contract(
        reference: &str,
        access_mode: AccessMode,
    ) -> Result<Self, PrepareFileWriteError> {
        match access_mode {
            AccessMode::Create => Ok(Self::Create),
            AccessMode::Write => Ok(Self::Write),
            AccessMode::Append => Ok(Self::Append),
            AccessMode::Delete => Ok(Self::Delete),
            AccessMode::Read => Err(PrepareFileWriteError::UnsupportedAccessMode {
                reference: reference.to_string(),
                access_mode,
            }),
        }
    }

    const fn as_contract(self) -> AccessMode {
        match self {
            Self::Create => AccessMode::Create,
            Self::Write => AccessMode::Write,
            Self::Append => AccessMode::Append,
            Self::Delete => AccessMode::Delete,
        }
    }

    const fn requires_payload(self) -> bool {
        !matches!(self, Self::Delete)
    }

    const fn is_delete(self) -> bool {
        matches!(self, Self::Delete)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrepareFileWriteError {
    UnsupportedAccessMode {
        reference: String,
        access_mode: AccessMode,
    },
    MissingPayloadForWrite {
        reference: String,
    },
}

impl PrepareFileWriteError {
    fn push_diagnostic(
        &self,
        reasons: &mut Vec<EffectApplicationReason>,
        diagnostics: &mut Vec<String>,
    ) {
        match self {
            Self::UnsupportedAccessMode {
                reference,
                access_mode,
            } => {
                reasons.push(EffectApplicationReason::UnsupportedAccessMode);
                diagnostics.push(format!(
                    "unsupported write access mode for {reference}: {access_mode:?}"
                ));
            }
            Self::MissingPayloadForWrite { reference } => {
                reasons.push(EffectApplicationReason::MissingPayloadForWrite);
                diagnostics.push(format!("missing payload for {reference}"));
            }
        }
    }
}

#[derive(Debug, Clone)]
struct OriginalFileState {
    target: PathBuf,
    physical_reference: String,
    existed: bool,
    content: Vec<u8>,
    installed_content: Option<Vec<u8>>,
}

fn validate_file_backed_reads_retained(
    root: &retained_dir::RetainedDirectory,
    effect: &ToolEffectContractDocument,
    reasons: &mut Vec<EffectApplicationReason>,
    diagnostics: &mut Vec<String>,
) {
    for read in &effect.tool_effect_contract.read_set {
        if !file_backed_target(read.target_kind) {
            continue;
        }
        let Some(expected_hash) = &read.expected_hash else {
            continue;
        };
        let Ok(physical_ref) = effect_target_physical_reference(read.target_kind, &read.reference)
        else {
            reasons.push(EffectApplicationReason::InvalidTargetPath);
            diagnostics.push(format!("invalid read target path {}", read.reference));
            continue;
        };
        match read_source_file(root, Path::new(&physical_ref)) {
            Ok(content) if sha256_content_hash(&content) == *expected_hash => {}
            Ok(_) | Err(_) => {
                reasons.push(EffectApplicationReason::ExpectedHashMismatch);
                diagnostics.push(format!("read freshness mismatch {}", read.reference));
            }
        }
    }
}

fn prepare_file_writes(
    root: &Path,
    publication_root: &retained_dir::RetainedDirectory,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    reasons: &mut Vec<EffectApplicationReason>,
    diagnostics: &mut Vec<String>,
) -> Option<Vec<PreparedWrite>> {
    let mut writes = Vec::new();

    for write in &effect.tool_effect_contract.write_set {
        if !file_backed_target(write.target_kind) {
            reasons.push(EffectApplicationReason::UnsupportedTargetKind);
            diagnostics.push(format!("unsupported target kind for {}", write.reference));
            continue;
        }
        let access_mode =
            match PreparedAccessMode::from_contract(&write.reference, write.access_mode) {
                Ok(access_mode) => access_mode,
                Err(error) => {
                    error.push_diagnostic(reasons, diagnostics);
                    continue;
                }
            };

        let (target, physical_reference) =
            match resolve_effect_target(root, write.target_kind, &write.reference) {
                Ok(resolved) => resolved,
                Err(AppendJsonLineError::ReservedStatePath { reserved, .. }) => {
                    reasons.push(EffectApplicationReason::ReservedStatePath);
                    diagnostics.push(format!(
                        "write target {} is reserved for EventLog TCB: {reserved:?}",
                        write.reference
                    ));
                    continue;
                }
                Err(_) => {
                    reasons.push(EffectApplicationReason::InvalidTargetPath);
                    diagnostics.push(format!("invalid write target path {}", write.reference));
                    continue;
                }
            };

        let payload = if access_mode.requires_payload() {
            if let Some(payload) = payload_for(payloads, &write.reference) {
                Some(payload)
            } else {
                PrepareFileWriteError::MissingPayloadForWrite {
                    reference: write.reference.clone(),
                }
                .push_diagnostic(reasons, diagnostics);
                continue;
            }
        } else {
            None
        };

        if let Some(payload) = payload {
            let actual_hash = sha256_content_hash(&payload.content);
            if actual_hash != payload.content_hash {
                reasons.push(EffectApplicationReason::PayloadHashMismatch);
                diagnostics.push(format!("payload hash mismatch for {}", write.reference));
                continue;
            }
        }

        let relative = Path::new(&physical_reference);
        let target_exists = match authority_leaf_exists(publication_root, relative) {
            Ok(exists) => exists,
            Err(error) => {
                reasons.push(EffectApplicationReason::InvalidTargetPath);
                diagnostics.push(format!(
                    "retained target inspection {} failed: {error}",
                    write.reference
                ));
                continue;
            }
        };
        match access_mode {
            PreparedAccessMode::Create if target_exists => {
                reasons.push(EffectApplicationReason::TargetExistsForCreate);
                diagnostics.push(format!("create target exists {}", write.reference));
                continue;
            }
            PreparedAccessMode::Write if !target_exists => {
                reasons.push(EffectApplicationReason::TargetMissingForWrite);
                diagnostics.push(format!("write target missing {}", write.reference));
                continue;
            }
            PreparedAccessMode::Write if write.expected_hash.is_none() => {
                reasons.push(EffectApplicationReason::MissingExpectedHashForOverwrite);
                diagnostics.push(format!(
                    "write target missing expected hash {}",
                    write.reference
                ));
                continue;
            }
            PreparedAccessMode::Delete if !target_exists => {
                reasons.push(EffectApplicationReason::TargetMissingForDelete);
                diagnostics.push(format!("delete target missing {}", write.reference));
                continue;
            }
            _ => {}
        }

        if let Some(expected_hash) = &write.expected_hash {
            match read_authority_file(publication_root, relative) {
                Ok(content) if sha256_content_hash(&content) == *expected_hash => {}
                Ok(_) | Err(_) => {
                    reasons.push(EffectApplicationReason::ExpectedHashMismatch);
                    diagnostics.push(format!("write freshness mismatch {}", write.reference));
                    continue;
                }
            }
        }

        let content = match (access_mode, payload) {
            (PreparedAccessMode::Create | PreparedAccessMode::Write, Some(payload)) => {
                payload.content.clone()
            }
            (PreparedAccessMode::Append, Some(payload)) => {
                let mut content = match read_authority_file(publication_root, relative) {
                    Ok(content) => content,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
                    Err(error) => {
                        reasons.push(EffectApplicationReason::InvalidTargetPath);
                        diagnostics.push(format!(
                            "retained append source {} could not be read exactly: {error}",
                            write.reference
                        ));
                        continue;
                    }
                };
                content.extend_from_slice(&payload.content);
                content
            }
            (PreparedAccessMode::Delete, _) => Vec::new(),
            (
                PreparedAccessMode::Create | PreparedAccessMode::Write | PreparedAccessMode::Append,
                None,
            ) => {
                PrepareFileWriteError::MissingPayloadForWrite {
                    reference: write.reference.clone(),
                }
                .push_diagnostic(reasons, diagnostics);
                continue;
            }
        };

        writes.push(PreparedWrite {
            reference: write.reference.clone(),
            physical_reference,
            target,
            target_kind: write.target_kind,
            access_mode,
            destructive: write.destructive,
            expected_hash: write.expected_hash.clone(),
            payload_content: payload.map(|payload| payload.content.clone()),
            content,
        });
    }

    if reasons.is_empty() {
        Some(writes)
    } else {
        None
    }
}

fn revalidate_prepared_writes_retained(
    root: &retained_dir::RetainedDirectory,
    writes: &mut [PreparedWrite],
    reasons: &mut Vec<EffectApplicationReason>,
    diagnostics: &mut Vec<String>,
) -> bool {
    let mut ok = true;
    for write in writes {
        if let Some(reserved) = reserved_state_path(&write.physical_reference) {
            ok = false;
            reasons.push(EffectApplicationReason::ReservedStatePath);
            diagnostics.push(format!(
                "final write validation blocked EventLog-reserved target {}: {reserved:?}",
                write.reference
            ));
            continue;
        }
        let relative = Path::new(&write.physical_reference);
        let target_exists = match authority_leaf_exists(root, relative) {
            Ok(exists) => exists,
            Err(error) => {
                ok = false;
                reasons.push(EffectApplicationReason::InvalidTargetPath);
                diagnostics.push(format!(
                    "retained target inspection {} failed: {error}",
                    write.reference
                ));
                continue;
            }
        };
        match write.access_mode {
            PreparedAccessMode::Create if target_exists => {
                ok = false;
                reasons.push(EffectApplicationReason::TargetExistsForCreate);
                diagnostics.push(format!("create target exists {}", write.reference));
                continue;
            }
            PreparedAccessMode::Write if !target_exists => {
                ok = false;
                reasons.push(EffectApplicationReason::TargetMissingForWrite);
                diagnostics.push(format!("write target missing {}", write.reference));
                continue;
            }
            PreparedAccessMode::Delete if !target_exists => {
                ok = false;
                reasons.push(EffectApplicationReason::TargetMissingForDelete);
                diagnostics.push(format!("delete target missing {}", write.reference));
                continue;
            }
            _ => {}
        }
        if let Some(expected_hash) = &write.expected_hash {
            match read_authority_file(root, relative) {
                Ok(content) if sha256_content_hash(&content) == *expected_hash => {}
                Ok(_) | Err(_) => {
                    ok = false;
                    reasons.push(EffectApplicationReason::ExpectedHashMismatch);
                    diagnostics.push(format!("write freshness mismatch {}", write.reference));
                    continue;
                }
            }
        }
        match write.access_mode {
            PreparedAccessMode::Create | PreparedAccessMode::Write => {
                let Some(payload_content) = &write.payload_content else {
                    ok = false;
                    PrepareFileWriteError::MissingPayloadForWrite {
                        reference: write.reference.clone(),
                    }
                    .push_diagnostic(reasons, diagnostics);
                    continue;
                };
                write.content = payload_content.clone();
            }
            PreparedAccessMode::Append => {
                let Some(payload_content) = &write.payload_content else {
                    ok = false;
                    PrepareFileWriteError::MissingPayloadForWrite {
                        reference: write.reference.clone(),
                    }
                    .push_diagnostic(reasons, diagnostics);
                    continue;
                };
                let mut content = match read_authority_file(root, relative) {
                    Ok(content) => content,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
                    Err(error) => {
                        ok = false;
                        reasons.push(EffectApplicationReason::InvalidTargetPath);
                        diagnostics.push(format!(
                            "retained append source {} changed or became unreadable: {error}",
                            write.reference
                        ));
                        continue;
                    }
                };
                content.extend_from_slice(payload_content);
                write.content = content;
            }
            PreparedAccessMode::Delete => write.content.clear(),
        }
    }
    ok
}

fn effect_target_metadata_records(
    effect: &ToolEffectContractDocument,
    writes: &[PreparedWrite],
) -> Vec<EffectTargetMetadataRecord> {
    writes
        .iter()
        .map(|write| EffectTargetMetadataRecord {
            schema_version: "0.1".to_string(),
            record_kind: EffectTargetMetadataRecordKind::EffectTarget,
            recorded_at: None,
            operation_id: effect.tool_effect_contract.operation_ref.clone(),
            effect_id: effect.tool_effect_contract.id.clone(),
            logical_ref: write.reference.clone(),
            physical_ref: write.physical_reference.clone(),
            target_kind: write.target_kind,
            access_mode: write.access_mode.as_contract(),
            content_hash: if write.access_mode.is_delete() {
                None
            } else {
                Some(sha256_content_hash(&write.content))
            },
            byte_len: write.content.len() as u64,
            actor_agent_id: effect.tool_effect_contract.actor.agent_id.clone(),
            actor_role: effect.tool_effect_contract.actor.role,
            destructive: write.destructive,
            redaction_hint: StableId("raw_content_not_indexed".to_string()),
        })
        .collect()
}

fn effect_metadata_authority_boundary() -> EffectMetadataAuthorityBoundary {
    EffectMetadataAuthorityBoundary {
        is_workflow_authority: false,
        allowed_uses: vec![
            EffectMetadataConsumerUse::Discovery,
            EffectMetadataConsumerUse::Diagnostics,
            EffectMetadataConsumerUse::HandoffContext,
        ],
        forbidden_authority: vec![
            EffectMetadataForbiddenAuthority::PhaseTransition,
            EffectMetadataForbiddenAuthority::RouteChange,
            EffectMetadataForbiddenAuthority::DecisionClose,
            EffectMetadataForbiddenAuthority::CompletionClose,
            EffectMetadataForbiddenAuthority::ReleaseReadiness,
            EffectMetadataForbiddenAuthority::StateMutation,
        ],
        required_authority_contracts: vec![
            "OperationContract".to_string(),
            "GateContract".to_string(),
            "DecisionCloseContract".to_string(),
            "CompletionContract".to_string(),
            "ClaimContract".to_string(),
            "RequestContract".to_string(),
        ],
        note:
            "effect metadata locates evidence and artifacts; it does not authorize workflow control"
                .to_string(),
    }
}

fn effect_metadata_adapter_presentation(
    options: &EffectMetadataContextBuildOptions,
) -> EffectMetadataAdapterPresentation {
    EffectMetadataAdapterPresentation {
        adapter_kind: options.adapter_kind,
        trigger: options.adapter_trigger,
        automatic_invocation_allowed: matches!(
            options.adapter_trigger,
            EffectMetadataAdapterTrigger::EvidenceDiscovery
                | EffectMetadataAdapterTrigger::Diagnostics
                | EffectMetadataAdapterTrigger::HandoffPreparation
        ),
        presentation_mode: EffectMetadataPresentationMode::AdvisoryContext,
        may_create_workflow_authority: false,
        required_output_treatment: vec![
            EffectMetadataOutputTreatment::PreserveAuthorityBoundary,
            EffectMetadataOutputTreatment::DoNotSummarizeAsNextAction,
            EffectMetadataOutputTreatment::DoNotMutateStateFromContext,
            EffectMetadataOutputTreatment::KeepRawContentOmitted,
        ],
    }
}

fn effect_target_metadata_query_matches(
    record: &EffectTargetMetadataRecord,
    query: &EffectTargetMetadataIndexQuery,
) -> bool {
    if let Some(logical_ref) = &query.logical_ref {
        if &record.logical_ref != logical_ref {
            return false;
        }
    }
    if let Some(effect_id) = &query.effect_id {
        if &record.effect_id != effect_id {
            return false;
        }
    }
    if let Some(operation_id) = &query.operation_id {
        if &record.operation_id != operation_id {
            return false;
        }
    }
    if let Some(target_kind) = query.target_kind {
        if record.target_kind != target_kind {
            return false;
        }
    }
    true
}

fn latest_effect_target_metadata_records(
    records: &[EffectTargetMetadataRecord],
) -> Vec<EffectTargetMetadataRecord> {
    let mut latest_index_by_key = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        latest_index_by_key.insert(effect_target_metadata_latest_key(record), index);
    }

    let mut latest_indices = latest_index_by_key.into_values().collect::<Vec<_>>();
    latest_indices.sort_unstable();
    latest_indices
        .into_iter()
        .filter_map(|index| {
            debug_assert!(
                index < records.len(),
                "latest metadata index must originate from records.iter().enumerate()"
            );
            records.get(index).cloned()
        })
        .collect()
}

fn effect_target_metadata_latest_key(record: &EffectTargetMetadataRecord) -> String {
    format!(
        "{}:{}",
        effect_target_kind_key(record.target_kind),
        record.logical_ref
    )
}

fn effect_target_kind_key(target_kind: EffectTargetKind) -> &'static str {
    match target_kind {
        EffectTargetKind::FilePath => "file_path",
        EffectTargetKind::Glob => "glob",
        EffectTargetKind::StateKey => "state_key",
        EffectTargetKind::ArtifactId => "artifact_id",
        EffectTargetKind::EvidenceId => "evidence_id",
        EffectTargetKind::LedgerStream => "ledger_stream",
        EffectTargetKind::RequestStream => "request_stream",
        EffectTargetKind::CompletionId => "completion_id",
    }
}

fn file_backed_target(target_kind: EffectTargetKind) -> bool {
    matches!(
        target_kind,
        EffectTargetKind::FilePath
            | EffectTargetKind::ArtifactId
            | EffectTargetKind::EvidenceId
            | EffectTargetKind::LedgerStream
            | EffectTargetKind::RequestStream
    )
}

fn payload_for<'a>(
    payloads: &'a [EffectApplicationPayload],
    target_ref: &str,
) -> Option<&'a EffectApplicationPayload> {
    payloads
        .iter()
        .find(|payload| payload.target_ref == target_ref)
}

struct RollbackWalTransaction<'a> {
    root: &'a Path,
    publication_root: &'a retained_dir::RetainedDirectory,
    wal_relative_path: &'a str,
    tx_id: &'a str,
    effect_id: StableId,
    originals: &'a [OriginalFileState],
    applied_refs: Vec<String>,
    reasons: Vec<EffectApplicationReason>,
    diagnostics: Vec<String>,
    validation_error_count: usize,
    validation_warning_count: usize,
}

fn rollback_wal_transaction_result(
    mut transaction: RollbackWalTransaction<'_>,
) -> EffectApplicationResult {
    let rollback = rollback_originals_for_publication(
        transaction.publication_root,
        transaction.originals,
        &mut transaction.diagnostics,
    );
    let rollback_recorded = append_effect_wal_record_for_publication(
        transaction.publication_root,
        transaction.root,
        transaction.wal_relative_path,
        EffectWalRecord::stage(
            transaction.tx_id,
            transaction.effect_id.clone(),
            EffectWalStage::RollbackComplete,
        ),
        // Rollback writes MUST be durable: losing the RollbackComplete marker
        // would cause the next recovery pass to undo work that was already
        // committed. See ADR-0009.
        WalDurability::SyncOnAppend,
    )
    .map_or_else(
        |error| {
            transaction
                .diagnostics
                .push(format!("failed to append WAL rollback record: {error}"));
            false
        },
        |_| true,
    );

    if !rollback || !rollback_recorded {
        push_unique_reason(
            &mut transaction.reasons,
            EffectApplicationReason::RollbackFailed,
        );
    }

    EffectApplicationResult {
        status: if rollback && rollback_recorded {
            EffectApplicationStatus::RolledBack
        } else {
            EffectApplicationStatus::RollbackFailed
        },
        effect_id: transaction.effect_id,
        applied_refs: transaction.applied_refs,
        metadata_records: Vec::new(),
        rolled_back: rollback,
        reasons: transaction.reasons,
        diagnostics: transaction.diagnostics,
        validation_error_count: transaction.validation_error_count,
        validation_warning_count: transaction.validation_warning_count,
    }
}

fn blocked_effect_application_result(
    effect_id: StableId,
    reasons: Vec<EffectApplicationReason>,
    diagnostics: Vec<String>,
    validation_error_count: usize,
    validation_warning_count: usize,
) -> EffectApplicationResult {
    EffectApplicationResult {
        status: EffectApplicationStatus::Blocked,
        effect_id,
        applied_refs: Vec::new(),
        metadata_records: Vec::new(),
        rolled_back: false,
        reasons,
        diagnostics,
        validation_error_count,
        validation_warning_count,
    }
}

fn push_unique_reason(reasons: &mut Vec<EffectApplicationReason>, reason: EffectApplicationReason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn capture_originals_retained(
    root: &retained_dir::RetainedDirectory,
    writes: &mut [PreparedWrite],
) -> io::Result<Vec<OriginalFileState>> {
    writes
        .iter_mut()
        .map(|write| {
            let relative = Path::new(&write.physical_reference);
            let (existed, content) = match read_authority_file(root, relative) {
                Ok(content) => (true, content),
                Err(source) if source.kind() == io::ErrorKind::NotFound => (false, Vec::new()),
                Err(source) => return Err(source),
            };
            let expected_presence = match write.access_mode {
                PreparedAccessMode::Create => Some(false),
                PreparedAccessMode::Write | PreparedAccessMode::Delete => Some(true),
                PreparedAccessMode::Append => None,
            };
            if expected_presence.is_some_and(|expected| existed != expected) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained target presence changed before exact before-image capture",
                ));
            }
            if write.expected_hash.as_ref().is_some_and(|expected| {
                !existed || sha256_content_hash(&content) != expected.as_str()
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "retained target digest changed before exact before-image capture",
                ));
            }
            if write.access_mode == PreparedAccessMode::Append {
                let payload = write.payload_content.as_deref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "retained append lost its exact payload binding",
                    )
                })?;
                write.content = content
                    .iter()
                    .copied()
                    .chain(payload.iter().copied())
                    .collect();
            }
            Ok(OriginalFileState {
                target: write.target.clone(),
                physical_reference: write.physical_reference.clone(),
                existed,
                content,
                installed_content: (!write.access_mode.is_delete()).then(|| write.content.clone()),
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ApplyPreparedWriteError {
    Io {
        action: &'static str,
        path: PathBuf,
        source: String,
    },
}

impl fmt::Display for ApplyPreparedWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => write!(formatter, "{action} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for ApplyPreparedWriteError {}

fn apply_prepared_write_for_publication(
    root: &retained_dir::RetainedDirectory,
    write: &PreparedWrite,
    original: &OriginalFileState,
) -> Result<(), ApplyPreparedWriteError> {
    if let Some(reserved) = reserved_state_path(&write.physical_reference) {
        return Err(ApplyPreparedWriteError::Io {
            action: "reserved EventLog target rejected",
            path: write.target.clone(),
            source: format!("{reserved:?}"),
        });
    }
    if original.physical_reference != write.physical_reference
        || original.target != write.target
        || original.installed_content.as_deref()
            != (!write.access_mode.is_delete()).then_some(write.content.as_slice())
    {
        return Err(ApplyPreparedWriteError::Io {
            action: "retained before-image binding rejected",
            path: write.target.clone(),
            source: "prepared write no longer matches its exact captured state".to_owned(),
        });
    }
    let relative = Path::new(&write.physical_reference);
    let expected = original.existed.then_some(original.content.as_slice());
    let result = match write.access_mode {
        PreparedAccessMode::Create | PreparedAccessMode::Write | PreparedAccessMode::Append => {
            atomic_replace_file_retained(root, relative, &write.content, expected)
        }
        PreparedAccessMode::Delete => {
            let expected = expected.ok_or_else(|| ApplyPreparedWriteError::Io {
                action: "retained delete binding rejected",
                path: write.target.clone(),
                source: "delete target was absent during exact before-image capture".to_owned(),
            })?;
            remove_authority_file_retained(root, relative, expected).and_then(|()| {
                relative
                    .parent()
                    .map_or_else(|| root.sync_root(), |parent| root.sync_directory(parent))
            })
        }
    };
    result.map_err(|source| ApplyPreparedWriteError::Io {
        action: "descriptor-relative publication",
        path: write.target.clone(),
        source: source.to_string(),
    })
}

fn rollback_originals_for_publication(
    root: &retained_dir::RetainedDirectory,
    originals: &[OriginalFileState],
    diagnostics: &mut Vec<String>,
) -> bool {
    let mut ok = true;
    for original in originals.iter().rev() {
        if let Some(reserved) = reserved_state_path(&original.physical_reference) {
            ok = false;
            diagnostics.push(format!(
                "rollback blocked EventLog-reserved target {}: {reserved:?}",
                original.target.display()
            ));
            continue;
        }
        let relative = Path::new(&original.physical_reference);
        let result = read_optional_authority_file(root, relative).and_then(|current| {
            let current_bytes = current.as_deref();
            let original_matches = if original.existed {
                current_bytes == Some(original.content.as_slice())
            } else {
                current_bytes.is_none()
            };
            if original_matches {
                return Ok(());
            }
            if current_bytes != original.installed_content.as_deref() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "rollback target matches neither the exact original nor installed state",
                ));
            }
            if original.existed {
                atomic_replace_file_retained(
                    root,
                    relative,
                    &original.content,
                    original.installed_content.as_deref(),
                )
            } else if let Some(installed) = original.installed_content.as_deref() {
                remove_authority_file_retained(root, relative, installed).and_then(|()| {
                    relative
                        .parent()
                        .map_or_else(|| root.sync_root(), |parent| root.sync_directory(parent))
                })
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "rollback has neither an original nor an exact installed file",
                ))
            }
        });
        if let Err(error) = result {
            ok = false;
            diagnostics.push(format!(
                "rollback {} failed: {error}",
                original.target.display()
            ));
        }
    }
    ok
}

fn read_source_file(
    root: &retained_dir::RetainedDirectory,
    relative: &Path,
) -> io::Result<Vec<u8>> {
    let mut file = root.open_leaf_read(relative, retained_dir::RetainedLeafPolicy::SourceRead)?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)?;
    Ok(content)
}

fn read_authority_file(
    root: &retained_dir::RetainedDirectory,
    relative: &Path,
) -> io::Result<Vec<u8>> {
    root.read_authority_bounded(relative, u64::MAX)
}

fn authority_leaf_exists(
    root: &retained_dir::RetainedDirectory,
    relative: &Path,
) -> io::Result<bool> {
    match root.open_leaf_read(relative, retained_dir::RetainedLeafPolicy::Authority) {
        Ok(file) => {
            drop(file);
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn validate_expected_authority_file(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    expected: Option<&[u8]>,
) -> io::Result<()> {
    match (expected, root.read_authority_bounded(target, u64::MAX)) {
        (Some(expected), Ok(actual)) if actual == expected => Ok(()),
        (None, Err(source)) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        (Some(_) | None, Ok(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "retained authority leaf changed after exact state capture",
        )),
        (_, Err(source)) => Err(source),
    }
}

fn read_optional_authority_file(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
) -> io::Result<Option<Vec<u8>>> {
    match root.read_authority_bounded(target, u64::MAX) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(source),
    }
}

fn atomic_replace_file_retained(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    content: &[u8],
    expected: Option<&[u8]>,
) -> io::Result<()> {
    if let Some(reserved) = target.to_str().and_then(reserved_state_path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("EventLog-reserved mutation target: {reserved:?}"),
        ));
    }
    let authority = root.retain_authority()?;
    let _cleanup_debt =
        authority.replace_file_with_validation(target, content, |directory, _, destination| {
            validate_expected_authority_file(directory, destination, expected)
        })?;
    Ok(())
}

fn remove_authority_file_retained(
    root: &retained_dir::RetainedDirectory,
    target: &Path,
    expected: &[u8],
) -> io::Result<()> {
    if let Some(reserved) = target.to_str().and_then(reserved_state_path) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("EventLog-reserved mutation target: {reserved:?}"),
        ));
    }
    let authority = root.retain_authority()?;
    let _cleanup_debt = authority.remove_file_with_validation(target, |directory, source| {
        validate_expected_authority_file(directory, source, Some(expected))
    })?;
    Ok(())
}

fn validate_effect_lock_scope(
    root: &Path,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
) -> Result<(), EffectReplayReconciliationError> {
    let expected_relative = normalized_effect_relative_path(expected_lock_relative_path)
        .ok_or_else(|| EffectReplayReconciliationError::InvalidRelativePath {
            field: "expected_lock_relative_path",
            path: expected_lock_relative_path.to_owned(),
        })?;
    if expected_relative != effect_lock.lock_relative_path {
        return Err(EffectReplayReconciliationError::LockScopeMismatch {
            expected: root.join(expected_relative),
            actual: effect_lock.path.clone(),
        });
    }
    let state_root = effect_boundary_state_root(root, expected_lock_relative_path);
    effect_lock
        .boundary
        .validate_root(&state_root)
        .map_err(|_| EffectReplayReconciliationError::LockScopeMismatch {
            expected: root.join(expected_lock_relative_path),
            actual: effect_lock.path.clone(),
        })?;
    let state_identity = effect_lock.state_root.identity();
    let retained_state_identity = if effect_uses_legacy_sidecar(expected_lock_relative_path) {
        effect_lock
            .root
            .open_directory(Path::new(".forge-method"))
            .and_then(|directory| directory.identity())
    } else {
        effect_lock.root.identity()
    };
    if !matches!(
        (state_identity, retained_state_identity),
        (Ok(state), Ok(retained)) if state == retained
    ) {
        return Err(EffectReplayReconciliationError::LockScopeMismatch {
            expected: root.join(expected_lock_relative_path),
            actual: effect_lock.path.clone(),
        });
    }
    let current_lock_identity = effect_lock
        .state_root
        .open_leaf_read(
            &effect_lock.state_lock_relative_path,
            retained_dir::RetainedLeafPolicy::Authority,
        )
        .and_then(|file| retained_dir::RetainedDirectory::identity_of(&file));
    if !current_lock_identity.is_ok_and(|identity| identity == effect_lock.lock_identity) {
        return Err(EffectReplayReconciliationError::LockScopeMismatch {
            expected: root.join(expected_lock_relative_path),
            actual: effect_lock.path.clone(),
        });
    }
    Ok(())
}

fn reconciliation_relative_path(
    field: &'static str,
    relative: &str,
) -> Result<PathBuf, EffectReplayReconciliationError> {
    if let Some(reserved) = reserved_state_path(relative) {
        return Err(EffectReplayReconciliationError::ReservedStatePath {
            field,
            path: relative.to_owned(),
            reserved,
        });
    }
    normalized_effect_relative_path(relative).ok_or_else(|| {
        EffectReplayReconciliationError::InvalidRelativePath {
            field,
            path: relative.to_owned(),
        }
    })
}

fn validate_effect_replay_binding(binding: &EffectReplayCommitBinding) -> Result<(), String> {
    if !is_lower_sha256_token(&binding.key_hash) {
        return Err("key_hash must be a lowercase sha256 token".to_owned());
    }
    if !is_lower_sha256_token(&binding.intent_digest) {
        return Err("intent_digest must be a lowercase sha256 token".to_owned());
    }
    if !is_lower_sha256_token(&binding.commit_digest) {
        return Err("commit_digest must be a lowercase sha256 token".to_owned());
    }
    if binding.reservation_revision == 0 {
        return Err("reservation_revision must be greater than zero".to_owned());
    }
    Ok(())
}

fn is_lower_sha256_token(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

fn repair_effect_wal_tail_retained(
    root: &retained_dir::RetainedDirectory,
    wal_relative: &Path,
    wal_path: &Path,
) -> Result<bool, EffectReplayReconciliationError> {
    let bytes = read_authority_file(root, wal_relative).map_err(|source| {
        EffectReplayReconciliationError::WalRead {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    if bytes.is_empty() || bytes.ends_with(b"\n") {
        return Ok(false);
    }
    let tail_start = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    let complete_final_record = std::str::from_utf8(&bytes[tail_start..])
        .ok()
        .and_then(|tail| serde_json::from_str::<EffectWalRecord>(tail).ok())
        .is_some();
    let mut file = root.open_read_write(wal_relative).map_err(|source| {
        EffectReplayReconciliationError::WalRepair {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let repair = if complete_final_record {
        file.seek(SeekFrom::End(0))
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|()| file.sync_all())
    } else {
        file.set_len(u64::try_from(tail_start).unwrap_or(u64::MAX))
            .and_then(|()| file.sync_all())
    };
    repair.map_err(|source| EffectReplayReconciliationError::WalRepair {
        path: wal_path.to_path_buf(),
        source: source.to_string(),
    })?;
    Ok(true)
}

fn read_effect_wal_records_strict_retained(
    root: &retained_dir::RetainedDirectory,
    wal_relative: &Path,
    wal_path: &Path,
) -> Result<Vec<EffectWalRecord>, EffectReplayReconciliationError> {
    let bytes = read_authority_file(root, wal_relative).map_err(|source| {
        EffectReplayReconciliationError::WalRead {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let content =
        String::from_utf8(bytes).map_err(|source| EffectReplayReconciliationError::WalParse {
            path: wal_path.to_path_buf(),
            line: 1,
            source: source.to_string(),
        })?;
    parse_effect_wal_records_strict(&content, wal_path)
}

fn parse_effect_wal_records_strict(
    content: &str,
    wal_path: &Path,
) -> Result<Vec<EffectWalRecord>, EffectReplayReconciliationError> {
    if !content.is_empty() && !content.ends_with('\n') {
        return Err(EffectReplayReconciliationError::WalParse {
            path: wal_path.to_path_buf(),
            line: content.lines().count().max(1),
            source: "effect WAL has a non-durable partial final line".to_owned(),
        });
    }
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<EffectWalRecord>(line).map_err(|source| {
                EffectReplayReconciliationError::WalParse {
                    path: wal_path.to_path_buf(),
                    line: index + 1,
                    source: source.to_string(),
                }
            })
        })
        .collect()
}

#[derive(Debug)]
struct EffectReplayProjection {
    effect_id: StableId,
    provenance: EffectExecutionProvenance,
    replay_binding: EffectReplayCommitBinding,
    committed: bool,
    rolled_back: bool,
    completed: bool,
}

fn project_pending_effect_replay_commits(
    records: &[EffectWalRecord],
) -> Result<Vec<PendingEffectReplayCommit>, EffectReplayReconciliationError> {
    let mut transactions = BTreeMap::<String, EffectReplayProjection>::new();
    for record in records {
        if record.stage == EffectWalStage::Begin {
            match (&record.execution_provenance, &record.replay_binding) {
                (None, None) => continue,
                (Some(provenance), Some(binding)) => {
                    provenance.verify().map_err(|source| {
                        EffectReplayReconciliationError::InvalidProvenance {
                            tx_id: record.tx_id.clone(),
                            source: source.to_string(),
                        }
                    })?;
                    validate_effect_replay_binding(binding).map_err(|reason| {
                        EffectReplayReconciliationError::InvalidReplayBinding {
                            tx_id: record.tx_id.clone(),
                            reason,
                        }
                    })?;
                    if transactions
                        .insert(
                            record.tx_id.clone(),
                            EffectReplayProjection {
                                effect_id: record.effect_id.clone(),
                                provenance: provenance.clone(),
                                replay_binding: binding.clone(),
                                committed: false,
                                rolled_back: false,
                                completed: false,
                            },
                        )
                        .is_some()
                    {
                        return Err(EffectReplayReconciliationError::ConflictingTransaction {
                            tx_id: record.tx_id.clone(),
                            reason: "multiple provenance-bound begin records".to_owned(),
                        });
                    }
                }
                _ => {
                    return Err(EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "begin must carry provenance and replay binding together"
                            .to_owned(),
                    });
                }
            }
            continue;
        }

        let Some(transaction) = transactions.get_mut(&record.tx_id) else {
            if record.stage == EffectWalStage::ReplayConsumed {
                return Err(EffectReplayReconciliationError::ConflictingTransaction {
                    tx_id: record.tx_id.clone(),
                    reason: "replay completion has no provenance-bound begin".to_owned(),
                });
            }
            continue;
        };
        if record.effect_id != transaction.effect_id {
            return Err(EffectReplayReconciliationError::ConflictingTransaction {
                tx_id: record.tx_id.clone(),
                reason: "effect id changed within transaction".to_owned(),
            });
        }
        match record.stage {
            EffectWalStage::Commit => {
                if transaction.committed || transaction.rolled_back {
                    return Err(EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "duplicate or post-rollback commit".to_owned(),
                    });
                }
                transaction.committed = true;
            }
            EffectWalStage::RollbackComplete | EffectWalStage::RecoveredRollback => {
                if transaction.committed || transaction.completed {
                    return Err(EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "rollback appears after commit or replay completion".to_owned(),
                    });
                }
                transaction.rolled_back = true;
            }
            EffectWalStage::ReplayConsumed => {
                if !transaction.committed || transaction.completed {
                    return Err(EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "replay completion requires one prior commit".to_owned(),
                    });
                }
                let binding = record.replay_binding.as_ref().ok_or_else(|| {
                    EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "replay completion is missing its binding".to_owned(),
                    }
                })?;
                let completion = record.replay_completion.as_ref().ok_or_else(|| {
                    EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "replay completion evidence is missing".to_owned(),
                    }
                })?;
                if binding != &transaction.replay_binding
                    || completion.key_hash != binding.key_hash
                    || completion.reservation_revision != binding.reservation_revision
                    || completion.consumed_revision
                        != binding.reservation_revision.saturating_add(1)
                {
                    return Err(EffectReplayReconciliationError::ConflictingTransaction {
                        tx_id: record.tx_id.clone(),
                        reason: "replay completion does not match the begin binding".to_owned(),
                    });
                }
                transaction.completed = true;
            }
            EffectWalStage::Begin | EffectWalStage::BeforeImage | EffectWalStage::WriteApplied => {}
        }
    }

    Ok(transactions
        .into_iter()
        .filter(|(_, transaction)| {
            transaction.committed && !transaction.rolled_back && !transaction.completed
        })
        .map(|(tx_id, transaction)| PendingEffectReplayCommit {
            tx_id,
            effect_id: transaction.effect_id,
            provenance: transaction.provenance,
            replay_binding: transaction.replay_binding,
        })
        .collect())
}

impl EffectWalRecord {
    fn begin(tx_id: &str, effect_id: StableId) -> Self {
        Self {
            schema_version: "0.1".to_string(),
            tx_id: tx_id.to_string(),
            stage: EffectWalStage::Begin,
            effect_id,
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        }
    }

    fn begin_with_authority(
        tx_id: &str,
        effect_id: StableId,
        execution_provenance: EffectExecutionProvenance,
        replay_binding: EffectReplayCommitBinding,
    ) -> Self {
        let mut record = Self::begin(tx_id, effect_id);
        record.execution_provenance = Some(execution_provenance);
        record.replay_binding = Some(replay_binding);
        record
    }

    fn before_image(
        tx_id: &str,
        effect_id: StableId,
        write: &PreparedWrite,
        original: &OriginalFileState,
    ) -> Self {
        Self {
            schema_version: "0.1".to_string(),
            tx_id: tx_id.to_string(),
            stage: EffectWalStage::BeforeImage,
            effect_id,
            target_ref: Some(write.reference.clone()),
            physical_target_ref: Some(write.physical_reference.clone()),
            target_metadata: None,
            original: Some(EffectWalOriginal {
                existed: original.existed,
                content: original.content.clone(),
                content_hash: sha256_content_hash(&original.content),
            }),
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        }
    }

    fn write_applied(
        tx_id: &str,
        effect: &ToolEffectContractDocument,
        write: &PreparedWrite,
    ) -> Self {
        Self {
            schema_version: "0.1".to_string(),
            tx_id: tx_id.to_string(),
            stage: EffectWalStage::WriteApplied,
            effect_id: effect.tool_effect_contract.id.clone(),
            target_ref: Some(write.reference.clone()),
            physical_target_ref: Some(write.physical_reference.clone()),
            target_metadata: Some(EffectWalTargetMetadata {
                operation_id: effect.tool_effect_contract.operation_ref.clone(),
                target_kind: write.target_kind,
                access_mode: write.access_mode.as_contract(),
                content_hash: if write.access_mode.is_delete() {
                    None
                } else {
                    Some(sha256_content_hash(&write.content))
                },
                byte_len: write.content.len() as u64,
                actor_agent_id: effect.tool_effect_contract.actor.agent_id.clone(),
                actor_role: effect.tool_effect_contract.actor.role,
                destructive: write.destructive,
                redaction_hint: StableId("raw_content_not_indexed".to_string()),
            }),
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        }
    }

    fn stage(tx_id: &str, effect_id: StableId, stage: EffectWalStage) -> Self {
        Self {
            schema_version: "0.1".to_string(),
            tx_id: tx_id.to_string(),
            stage,
            effect_id,
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: None,
            replay_completion: None,
        }
    }

    fn replay_consumed(
        tx_id: &str,
        effect_id: StableId,
        replay_binding: EffectReplayCommitBinding,
        replay_completion: EffectReplayCompletion,
    ) -> Self {
        Self {
            schema_version: "0.1".to_owned(),
            tx_id: tx_id.to_owned(),
            stage: EffectWalStage::ReplayConsumed,
            effect_id,
            target_ref: None,
            physical_target_ref: None,
            target_metadata: None,
            original: None,
            diagnostic: None,
            execution_provenance: None,
            replay_binding: Some(replay_binding),
            replay_completion: Some(replay_completion),
        }
    }
}

#[cfg(test)]
fn pause_after_durable_effect_begin_for_test(durability: WalDurability) {
    if durability != WalDurability::SyncOnAppend {
        return;
    }
    let Some(directory) = std::env::var_os("FORGE_EFFECT_TEST_PAUSE_AFTER_BEGIN") else {
        return;
    };
    let directory = PathBuf::from(directory);
    fs::write(directory.join("begin-durable"), b"ready")
        .expect("publish durable effect-begin test marker");
    let release = directory.join("release");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !release.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for effect publication test release"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
// Callers pass freshly-built `EffectWalRecord` values, so taking ownership
// keeps the call sites concise without forcing a binding just to take a
// reference.
//
// `durability` threads ADR-0009 through the WAL append path. Recovery and
// rollback callers hard-code `WalDurability::SyncOnAppend` (durability is
// load-bearing there); the apply path threads the caller's choice so
// benchmarks and tests can opt into `NoSync`.
#[allow(clippy::needless_pass_by_value)]
fn append_effect_wal_record_for_publication(
    publication_root: &retained_dir::RetainedDirectory,
    root: &Path,
    wal_relative_path: &str,
    record: EffectWalRecord,
    durability: WalDurability,
) -> Result<PathBuf, AppendJsonLineError> {
    if let Some(reserved) = reserved_state_path(wal_relative_path) {
        return Err(AppendJsonLineError::ReservedStatePath {
            path: wal_relative_path.to_owned(),
            reserved,
        });
    }
    let relative = normalized_effect_relative_path(wal_relative_path).ok_or_else(|| {
        AppendJsonLineError::InvalidRelativePath {
            path: wal_relative_path.to_owned(),
        }
    })?;
    let display_path = root.join(&relative);
    let mut line =
        serde_json::to_vec(&record).map_err(|source| AppendJsonLineError::Serialize {
            path: display_path.clone(),
            source: source.to_string(),
        })?;
    line.push(b'\n');
    if let Some(reserved) = reserved_state_path(wal_relative_path) {
        return Err(AppendJsonLineError::ReservedStatePath {
            path: wal_relative_path.to_owned(),
            reserved,
        });
    }
    let mut file = publication_root
        .open_read_write_create(&relative)
        .map_err(|source| AppendJsonLineError::OpenFile {
            path: display_path.clone(),
            source: source.to_string(),
        })?;
    file.seek(SeekFrom::End(0))
        .and_then(|_| file.write_all(&line))
        .and_then(|()| match durability {
            WalDurability::SyncOnAppend => file.sync_data(),
            WalDurability::NoSync => Ok(()),
        })
        .map_err(|source| AppendJsonLineError::Write {
            path: display_path.clone(),
            source: source.to_string(),
        })?;
    Ok(display_path)
}
fn reserved_wal_target(records: &[EffectWalRecord]) -> Option<(&str, ReservedStatePath)> {
    records.iter().find_map(|record| {
        record
            .physical_target_ref
            .as_deref()
            .or(record.target_ref.as_deref())
            .and_then(reserved_state_path)
            .map(|reserved| (record.tx_id.as_str(), reserved))
    })
}
fn incomplete_wal_transactions(records: &[EffectWalRecord]) -> Vec<String> {
    let mut begun = Vec::new();
    let mut closed = HashSet::new();

    for record in records {
        if record.stage == EffectWalStage::Begin && !begun.contains(&record.tx_id) {
            begun.push(record.tx_id.clone());
        }
        if matches!(
            record.stage,
            EffectWalStage::Commit
                | EffectWalStage::RollbackComplete
                | EffectWalStage::RecoveredRollback
        ) {
            closed.insert(record.tx_id.clone());
        }
    }

    begun
        .into_iter()
        .filter(|tx_id| !closed.contains(tx_id))
        .collect()
}

fn provenance_bound_transaction_ids(records: &[EffectWalRecord]) -> HashSet<String> {
    records
        .iter()
        .filter(|record| {
            record.stage == EffectWalStage::Begin
                && (record.execution_provenance.is_some() || record.replay_binding.is_some())
        })
        .map(|record| record.tx_id.clone())
        .collect()
}

fn effect_target_metadata_records_from_committed_wal(
    records: &[EffectWalRecord],
    recorded_at: Option<&str>,
) -> Vec<EffectTargetMetadataRecord> {
    let committed: HashSet<_> = records
        .iter()
        .filter(|record| record.stage == EffectWalStage::Commit)
        .map(|record| record.tx_id.as_str())
        .collect();

    records
        .iter()
        .filter(|record| {
            record.stage == EffectWalStage::WriteApplied
                && committed.contains(record.tx_id.as_str())
        })
        .filter_map(|record| {
            let logical_ref = record.target_ref.clone()?;
            let metadata = record.target_metadata.clone()?;
            Some(EffectTargetMetadataRecord {
                schema_version: "0.1".to_string(),
                record_kind: EffectTargetMetadataRecordKind::EffectTarget,
                recorded_at: recorded_at.map(str::to_string),
                operation_id: metadata.operation_id,
                effect_id: record.effect_id.clone(),
                logical_ref: logical_ref.clone(),
                physical_ref: record.physical_target_ref.clone().unwrap_or(logical_ref),
                target_kind: metadata.target_kind,
                access_mode: metadata.access_mode,
                content_hash: metadata.content_hash,
                byte_len: metadata.byte_len,
                actor_agent_id: metadata.actor_agent_id,
                actor_role: metadata.actor_role,
                destructive: metadata.destructive,
                redaction_hint: metadata.redaction_hint,
            })
        })
        .collect()
}

fn rollback_wal_before_images_retained(
    root: &retained_dir::RetainedDirectory,
    before_images: &[&EffectWalRecord],
    diagnostics: &mut Vec<String>,
) -> bool {
    let mut ok = true;
    for record in before_images.iter().rev() {
        let Some(target_ref) = &record.target_ref else {
            continue;
        };
        let Some(original) = &record.original else {
            continue;
        };
        let physical_target_ref = record.physical_target_ref.as_ref().unwrap_or(target_ref);
        if let Some(reserved) = reserved_state_path(physical_target_ref) {
            diagnostics.push(format!(
                "WAL rollback blocked EventLog-reserved target {physical_target_ref}: {reserved:?}"
            ));
            ok = false;
            continue;
        }
        let Some(relative) = normalized_effect_relative_path(physical_target_ref) else {
            diagnostics.push(format!("invalid WAL target path {physical_target_ref}"));
            ok = false;
            continue;
        };
        let result = if original.existed {
            if sha256_content_hash(&original.content) == original.content_hash {
                read_optional_authority_file(root, &relative).and_then(|current| {
                    if current.as_deref() == Some(original.content.as_slice()) {
                        Ok(())
                    } else {
                        atomic_replace_file_retained(
                            root,
                            &relative,
                            &original.content,
                            current.as_deref(),
                        )
                    }
                })
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "WAL before image hash mismatch",
                ))
            }
        } else {
            read_optional_authority_file(root, &relative).and_then(|current| match current {
                None => Ok(()),
                Some(current) => remove_authority_file_retained(root, &relative, &current)
                    .and_then(|()| {
                        relative
                            .parent()
                            .map_or_else(|| root.sync_root(), |parent| root.sync_directory(parent))
                    }),
            })
        };
        if let Err(error) = result {
            diagnostics.push(format!(
                "WAL rollback {physical_target_ref} failed: {error}"
            ));
            ok = false;
        }
    }
    ok
}

fn effect_uses_legacy_sidecar(authority_relative_path: &str) -> bool {
    matches!(
        Path::new(authority_relative_path).components().next(),
        Some(Component::Normal(component)) if component == ".forge-method"
    )
}

fn effect_boundary_state_root(root: &Path, authority_relative_path: &str) -> PathBuf {
    if effect_uses_legacy_sidecar(authority_relative_path) {
        root.join(".forge-method")
    } else {
        root.to_path_buf()
    }
}

fn retained_effect_publication_root(
    boundary: &impl producer_quiescence::ProducerBoundary,
    root: &Path,
    authority_relative_path: &str,
) -> Result<retained_dir::RetainedDirectory, String> {
    let state_root_path = effect_boundary_state_root(root, authority_relative_path);
    let lease = producer_quiescence::BoundaryLease::from_boundary(boundary, &state_root_path)
        .map_err(|error| error.to_string())?;
    let retained_state_root = lease.retained_root().map_err(|error| error.to_string())?;
    if !effect_uses_legacy_sidecar(authority_relative_path) {
        return Ok(retained_state_root);
    }

    // The boundary is rooted at `.forge-method` for legacy paths, but effect
    // targets may live anywhere under the project root. Open the parent only
    // after authority is held and cross-bind its retained child to the exact
    // state-root inode. An A→B whole-root swap therefore yields A or fails.
    let retained_root = retained_dir::RetainedDirectory::open_root(root)
        .map_err(|error| format!("retain effect publication root {}: {error}", root.display()))?;
    let contained_identity = retained_root
        .open_directory(Path::new(".forge-method"))
        .and_then(|directory| directory.identity())
        .map_err(|error| format!("inspect retained effect state root: {error}"))?;
    let authority_identity = retained_state_root
        .identity()
        .map_err(|error| format!("inspect effect authority identity: {error}"))?;
    if contained_identity != authority_identity {
        return Err(format!(
            "retained project root does not contain effect authority {}",
            state_root_path.display()
        ));
    }
    Ok(retained_root)
}

/// The only generic Store policy for EventLog-owned state artifacts. It has no
/// caller-supplied exception: mutation paths call it before opening a target
/// and again at their final retained-descriptor write boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReservedStateMutationError {
    Invalid,
    Reserved(ReservedStatePath),
}

fn reject_reserved_state_mutation(
    relative_path: &str,
) -> Result<PathBuf, ReservedStateMutationError> {
    let normalized = normalize_state_relative_path(relative_path)
        .map_err(|_| ReservedStateMutationError::Invalid)?;
    classify_reserved_state_path(&normalized).map_or(Ok(normalized), |reserved| {
        Err(ReservedStateMutationError::Reserved(reserved))
    })
}

fn reserved_state_path(relative_path: &str) -> Option<ReservedStatePath> {
    match reject_reserved_state_mutation(relative_path) {
        Err(ReservedStateMutationError::Reserved(reserved)) => Some(reserved),
        Ok(_) | Err(ReservedStateMutationError::Invalid) => None,
    }
}
fn normalized_effect_relative_path(relative_path: &str) -> Option<PathBuf> {
    if normalize_state_relative_path(relative_path).is_err() {
        return None;
    }
    if reserved_state_path(relative_path).is_some() {
        return None;
    }
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(forbidden_relative_component)
    {
        return None;
    }
    let normalized = path.components().collect::<PathBuf>();
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

fn effect_state_relative_path(relative_path: &Path, legacy_sidecar: bool) -> Option<PathBuf> {
    if legacy_sidecar {
        relative_path
            .strip_prefix(Path::new(".forge-method"))
            .ok()
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
    } else {
        Some(relative_path.to_path_buf())
    }
}

fn acquire_effect_store_lock_inner(
    root: &Path,
    lock_relative_path: &str,
    try_only: bool,
    allow_reserved_eventlog_lock: bool,
    boundary: producer_quiescence::BoundaryLease,
    validate_namespace_root: bool,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    let reserved = reserved_state_path(lock_relative_path);
    if let Some(reserved_path) = reserved {
        let is_designated_lock = matches!(
            reserved_path,
            ReservedStatePath::Artifact(artifact) if !artifact.is_log()
        );
        if !allow_reserved_eventlog_lock || !is_designated_lock {
            return Err(EffectStoreLockError::ReservedStatePath {
                path: lock_relative_path.to_owned(),
                reserved: reserved_path,
            });
        }
    }
    let relative = if reserved.is_some() {
        let path = Path::new(lock_relative_path);
        if path.as_os_str().is_empty()
            || path.is_absolute()
            || path.components().any(forbidden_relative_component)
        {
            return Err(EffectStoreLockError::InvalidRelativePath {
                path: lock_relative_path.to_owned(),
            });
        }
        path.components().collect::<PathBuf>()
    } else {
        normalized_effect_relative_path(lock_relative_path).ok_or_else(|| {
            EffectStoreLockError::InvalidRelativePath {
                path: lock_relative_path.to_owned(),
            }
        })?
    };
    let legacy_sidecar = effect_uses_legacy_sidecar(lock_relative_path);
    let state_relative =
        effect_state_relative_path(&relative, legacy_sidecar).ok_or_else(|| {
            EffectStoreLockError::InvalidRelativePath {
                path: lock_relative_path.to_string(),
            }
        })?;
    let path = root.join(&relative);
    boundary
        .require_effect_authority()
        .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    let state_root = boundary
        .retained_root()
        .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    let retained_root = if legacy_sidecar {
        let retained_root = retained_dir::RetainedDirectory::open_root(root).map_err(|source| {
            EffectStoreLockError::OpenFile {
                path: root.to_path_buf(),
                source: source.to_string(),
            }
        })?;
        let retained_sidecar_identity = retained_root
            .open_directory(Path::new(".forge-method"))
            .and_then(|directory| directory.identity());
        if !retained_sidecar_identity
            .is_ok_and(|identity| state_root.identity().is_ok_and(|state| state == identity))
        {
            return Err(EffectStoreLockError::OpenFile {
                path: root.join(".forge-method"),
                source: "retained effect root does not contain the boundary state root".to_owned(),
            });
        }
        retained_root
    } else {
        boundary
            .retained_root()
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?
    };
    let parent =
        state_relative
            .parent()
            .ok_or_else(|| EffectStoreLockError::InvalidRelativePath {
                path: lock_relative_path.to_string(),
            })?;
    if !parent.as_os_str().is_empty() {
        state_root
            .create_dir_all(parent)
            .map_err(|source| EffectStoreLockError::CreateDir {
                path: path.parent().unwrap_or(root).to_path_buf(),
                source: source.to_string(),
            })?;
    }
    if !allow_reserved_eventlog_lock {
        if let Some(reserved) = reserved_state_path(lock_relative_path) {
            return Err(EffectStoreLockError::ReservedStatePath {
                path: lock_relative_path.to_owned(),
                reserved,
            });
        }
    }
    let file = state_root
        .open_read_write_create(&state_relative)
        .map_err(|source| EffectStoreLockError::OpenFile {
            path: path.clone(),
            source: source.to_string(),
        })?;
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err(EffectStoreLockError::OpenFile {
            path,
            source: "effect lock is not a regular file".to_owned(),
        });
    }
    if try_only {
        match FileExt::try_lock(&file) {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(EffectStoreLockError::WouldBlock { path });
            }
            Err(TryLockError::Error(source)) => {
                return Err(EffectStoreLockError::Lock {
                    path,
                    source: source.to_string(),
                });
            }
        }
    } else {
        acquire_effect_store_lock_with_deadline(&file, &path)?;
    }
    let lock_identity = retained_dir::RetainedDirectory::identity_of(&file).map_err(|source| {
        EffectStoreLockError::OpenFile {
            path: path.clone(),
            source: source.to_string(),
        }
    })?;
    if validate_namespace_root {
        let boundary_root = effect_boundary_state_root(root, lock_relative_path);
        boundary
            .validate_root(&boundary_root)
            .map_err(|source| EffectStoreLockError::ProducerBoundary { source })?;
    }
    Ok(EffectStoreLock {
        file,
        path,
        lock_relative_path: relative,
        state_lock_relative_path: state_relative,
        root: retained_root,
        state_root,
        lock_identity,
        boundary,
    })
}

const EFFECT_STORE_LOCK_RETRY_ATTEMPTS: u32 = 60;

fn acquire_effect_store_lock_with_deadline(
    file: &File,
    path: &Path,
) -> Result<(), EffectStoreLockError> {
    for attempt in 0..EFFECT_STORE_LOCK_RETRY_ATTEMPTS {
        match FileExt::try_lock(file) {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) => {
                let shift = attempt.min(5);
                let backoff_ms = 2_u64.checked_shl(shift).unwrap_or(64);
                std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
            }
            Err(TryLockError::Error(source)) => {
                return Err(EffectStoreLockError::Lock {
                    path: path.to_path_buf(),
                    source: source.to_string(),
                });
            }
        }
    }
    Err(EffectStoreLockError::WouldBlock {
        path: path.to_path_buf(),
    })
}

fn nested_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str()
}

fn file_name_is(path: &Path, expected: &str) -> bool {
    path.file_name().and_then(|value| value.to_str()) == Some(expected)
}

fn repo_relative(root: &Path, path: &Path) -> Result<String, ReferenceIndexBuildError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| ReferenceIndexBuildError::PathOutsideRoot {
            root: root.to_path_buf(),
            path: path.to_path_buf(),
        })
}

fn diagnostic_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn temp_root(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-core-store-{test_name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
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
    fn store_project_tree_anchor_transfer_survives_fresh_locks_and_rejects_replacement() {
        const LOCK: &str = "locks/project-tree-anchor.lock";
        let store_root = temp_root("project-tree-anchor-store");
        let project_root = temp_root("project-tree-anchor-project");
        fs::create_dir_all(project_root.join("src")).expect("create project source directory");
        let project_file = project_root.join("src/lib.rs");

        let first_store =
            RetainedEffectStoreRoot::acquire(&store_root).expect("retain first Store root");
        let first_lock = first_store
            .acquire_effect_store_lock(LOCK)
            .expect("acquire first Store lock");
        let anchorless_project =
            retained_project_tree::RetainedProjectTree::capture(&project_root, 32, 4096)
                .expect("capture project without regular files");
        assert!(first_lock
            .retain_project_tree_anchors(
                &anchorless_project,
                Path::new("project-tree-anchors/anchorless"),
            )
            .is_err());
        drop(anchorless_project);

        fs::write(&project_file, b"pub fn exact() {}\n").expect("write exact project file");
        let first_project =
            retained_project_tree::RetainedProjectTree::capture_allowing_store_owned_file_anchors(
                &project_root,
                32,
                4096,
            )
            .expect("capture first project tree");
        first_project
            .revalidate_without_store_owned_file_anchors()
            .expect("initial project tree is pristine before anchor transfer");
        let first_anchors = first_lock
            .retain_project_tree_anchors(&first_project, Path::new("project-tree-anchors/selected"))
            .expect("transfer project tree into Store anchors");
        let binding_bytes = serde_json_canonicalizer::to_vec(first_anchors.binding())
            .expect("encode durable project anchor binding");
        first_anchors
            .revalidate()
            .expect("revalidate first anchors");
        drop(first_anchors);
        drop(first_project);
        drop(first_lock);
        drop(first_store);

        let binding: retained_project_tree::RetainedProjectAnchorBinding =
            serde_json::from_slice(&binding_bytes).expect("decode durable project anchor binding");
        let second_store =
            RetainedEffectStoreRoot::acquire(&store_root).expect("retain second Store root");
        let second_lock = second_store
            .acquire_effect_store_lock(LOCK)
            .expect("acquire fresh Store lock");
        let second_project =
            retained_project_tree::RetainedProjectTree::capture_allowing_store_owned_file_anchors(
                &project_root,
                32,
                4096,
            )
            .expect("capture project carrying Store anchor links");
        let reopened = second_lock
            .open_project_tree_anchors(&second_project, &binding)
            .expect("reopen anchors under fresh Store lock");
        reopened.revalidate().expect("revalidate reopened anchors");
        drop(reopened);
        drop(second_project);
        drop(second_lock);
        drop(second_store);

        fs::remove_file(&project_file).expect("remove exact project file name");
        fs::write(&project_file, b"pub fn exact() {}\n").expect("write byte-identical replacement");
        let third_store =
            RetainedEffectStoreRoot::acquire(&store_root).expect("retain third Store root");
        let third_lock = third_store
            .acquire_effect_store_lock(LOCK)
            .expect("acquire replacement-check Store lock");
        let replacement =
            retained_project_tree::RetainedProjectTree::capture_allowing_store_owned_file_anchors(
                &project_root,
                32,
                4096,
            )
            .expect("capture byte-identical replacement project");
        assert!(third_lock
            .open_project_tree_anchors(&replacement, &binding)
            .is_err());

        drop(replacement);
        drop(third_lock);
        drop(third_store);
        fs::remove_dir_all(project_root).expect("clean project root");
        fs::remove_dir_all(store_root).expect("clean Store root");
    }

    #[cfg(unix)]
    #[test]
    fn hard_linked_effect_authority_cleanup_preserves_every_external_link_source() {
        const WAL: &str = "wal/effects.ndjson";
        const INDEX: &str = "metadata/effects.ndjson";
        const EFFECT_LOCK: &str = "locks/effects.lock";
        let root = temp_root("hard-linked-effect-authority");
        let outside_wal = root.join("outside-wal");
        let outside_index = root.join("outside-index");
        let outside_delete = root.join("outside-delete");
        fs::write(&outside_wal, b"retained wal sentinel\n").expect("write WAL source");
        fs::write(&outside_index, b"retained index sentinel\n").expect("write index source");
        fs::write(&outside_delete, b"retained delete sentinel\n").expect("write delete source");
        fs::create_dir_all(root.join("wal")).expect("create WAL parent");
        fs::create_dir_all(root.join("metadata")).expect("create index parent");
        fs::create_dir_all(root.join("out")).expect("create delete parent");
        fs::hard_link(&outside_wal, root.join(WAL)).expect("link WAL authority");
        fs::hard_link(&outside_index, root.join(INDEX)).expect("link index authority");
        fs::hard_link(&outside_delete, root.join("out/delete.txt")).expect("link delete authority");
        let wal_before = fs::read(&outside_wal).expect("capture WAL source");
        let index_before = fs::read(&outside_index).expect("capture index source");
        let delete_before = fs::read(&outside_delete).expect("capture delete source");

        assert_eq!(
            recover_effect_wal(&root, WAL).status,
            EffectWalRecoveryStatus::RecoveryFailed
        );
        assert_eq!(
            rebuild_effect_target_metadata_index(&root, WAL, INDEX, None).status,
            EffectTargetMetadataIndexRebuildStatus::Failed
        );
        assert_eq!(
            compact_effect_wal(&root, WAL).status,
            EffectWalCompactionStatus::Failed
        );
        let lock = acquire_effect_store_lock(&root, EFFECT_LOCK).expect("acquire effect lock");
        assert_eq!(
            repair_effect_wal_tail_under_lock(&root, &lock, EFFECT_LOCK, WAL),
            Ok(false),
            "a newline-complete retained alias needs no tail mutation"
        );
        drop(lock);
        assert_eq!(
            query_effect_target_metadata_index(
                &root,
                INDEX,
                &EffectTargetMetadataIndexQuery::default()
            )
            .status,
            EffectTargetMetadataIndexQueryStatus::Failed
        );

        let publication_root = retained_dir::RetainedDirectory::open_root(&root)
            .expect("open retained publication root");
        let delete_write = PreparedWrite {
            reference: "out/delete.txt".to_owned(),
            physical_reference: "out/delete.txt".to_owned(),
            target: root.join("out/delete.txt"),
            target_kind: EffectTargetKind::FilePath,
            access_mode: PreparedAccessMode::Delete,
            destructive: true,
            expected_hash: None,
            payload_content: None,
            content: Vec::new(),
        };
        let delete_original = OriginalFileState {
            target: root.join("out/delete.txt"),
            physical_reference: "out/delete.txt".to_owned(),
            existed: true,
            content: delete_before.clone(),
            installed_content: None,
        };
        assert!(apply_prepared_write_for_publication(
            &publication_root,
            &delete_write,
            &delete_original,
        )
        .is_ok());
        let mut diagnostics = Vec::new();
        assert!(rollback_originals_for_publication(
            &publication_root,
            &[OriginalFileState {
                target: root.join("out/delete.txt"),
                physical_reference: "out/delete.txt".to_owned(),
                existed: false,
                content: Vec::new(),
                installed_content: None,
            }],
            &mut diagnostics,
        ));
        assert_eq!(
            fs::read(&outside_wal).expect("read unchanged WAL source"),
            wal_before
        );
        assert_eq!(
            fs::read(&outside_index).expect("read unchanged index source"),
            index_before
        );
        assert_eq!(
            fs::read(&outside_delete).expect("read unchanged delete source"),
            delete_before
        );
        fs::remove_dir_all(root).expect("clean hard-link authority root");
    }

    fn wait_for_test_path(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {}",
                path.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn namespace_split_test_effect() -> ToolEffectContractDocument {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("contracts/effects/story-artifact-write-effect.yaml");
        let mut effect: ToolEffectContractDocument =
            yaml_serde::from_str(&fs::read_to_string(fixture).expect("read effect fixture"))
                .expect("parse effect fixture");
        effect.tool_effect_contract.actor.agent_id = StableId("namespace-worker".to_owned());
        effect
            .tool_effect_contract
            .read_set
            .retain(|read| read.target_kind != EffectTargetKind::FilePath);
        effect.tool_effect_contract.write_set.truncate(1);
        let write = &mut effect.tool_effect_contract.write_set[0];
        write.reference = "out/committed.txt".to_owned();
        write.target_kind = EffectTargetKind::FilePath;
        effect
    }

    #[cfg(unix)]
    #[test]
    fn effect_namespace_subprocess_entrypoint() {
        let Some(root) = std::env::var_os("FORGE_EFFECT_NAMESPACE_WORKER_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let content = b"single-authority\n".to_vec();
        let payload = EffectApplicationPayload {
            target_ref: "out/committed.txt".to_owned(),
            content_hash: sha256_content_hash(&content),
            content,
        };
        let result = if std::env::var_os("FORGE_EFFECT_AMBIENT_WRAPPER").is_some() {
            apply_file_effect_transaction_with_wal(
                &root,
                &namespace_split_test_effect(),
                &[payload],
                ".forge-method/wal/effects.ndjson",
                "tx-ambient-root-swap",
            )
        } else {
            if let Some(attempted) = std::env::var_os("FORGE_EFFECT_AUTHORITY_ATTEMPTED") {
                fs::write(PathBuf::from(attempted), b"attempted")
                    .expect("publish authority-attempt marker");
            }
            let lock = acquire_effect_store_lock(&root, ".forge-method/locks/effects.lock")
                .expect("worker effect lock");
            if let Some(acquired) = std::env::var_os("FORGE_EFFECT_NAMESPACE_ACQUIRED") {
                fs::write(PathBuf::from(acquired), b"acquired").expect("publish acquired marker");
                return;
            }
            apply_file_effect_transaction_with_provenance_under_lock(
                &root,
                &lock,
                ".forge-method/locks/effects.lock",
                &namespace_split_test_effect(),
                &[payload],
                ".forge-method/wal/effects.ndjson",
                "tx-namespace-split",
                EffectExecutionProvenance::new(serde_json::json!({"schema_version": "0.1"}))
                    .expect("provenance"),
                EffectReplayCommitBinding::new(
                    sha256_content_hash(b"key"),
                    sha256_content_hash(b"intent"),
                    sha256_content_hash(b"commit"),
                    1,
                ),
            )
        };
        assert_eq!(
            result.status,
            EffectApplicationStatus::Applied,
            "{result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_state_root_authority_blocks_recreated_lock_directory_mid_publication() {
        const STATE_ROOT: &str = ".forge-method";
        const LOCKS: &str = ".forge-method/locks";
        const LOCK: &str = ".forge-method/locks/effects.lock";
        const WAL: &str = ".forge-method/wal/effects.ndjson";
        let root = temp_root("mid-publication-lock-directory-replacement");
        fs::create_dir_all(root.join(STATE_ROOT)).expect("state root");
        fs::create_dir_all(root.join("out")).expect("target parent");
        let hook = root.join("hook");
        fs::create_dir(&hook).expect("hook directory");

        let mut first = Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("tests::effect_namespace_subprocess_entrypoint")
            .arg("--nocapture")
            .env("FORGE_EFFECT_NAMESPACE_WORKER_ROOT", &root)
            .env("FORGE_EFFECT_TEST_PAUSE_AFTER_BEGIN", &hook)
            .spawn()
            .expect("spawn first effect worker");
        wait_for_test_path(&hook.join("begin-durable"));

        let wal_before = fs::read(root.join(WAL)).expect("durable begin WAL");
        let records: Vec<EffectWalRecord> = String::from_utf8(wal_before.clone())
            .expect("UTF-8 begin WAL")
            .lines()
            .map(|line| serde_json::from_str(line).expect("parse begin record"))
            .collect();
        assert_eq!(
            records.len(),
            1,
            "worker must pause after first publication"
        );
        assert_eq!(records[0].stage, EffectWalStage::Begin);
        assert!(!root.join("out/committed.txt").exists());

        fs::remove_file(root.join(LOCK)).expect("unlink held effect lock");
        fs::remove_file(
            root.join(STATE_ROOT)
                .join(producer_quiescence::PRODUCER_GATE_LOCK),
        )
        .expect("unlink held producer gate");
        fs::remove_file(
            root.join(STATE_ROOT)
                .join(producer_quiescence::PRODUCER_DRAIN_LOCK),
        )
        .expect("unlink held producer drain");
        fs::remove_dir(root.join(LOCKS)).expect("remove retained locks directory");
        fs::create_dir(root.join(LOCKS)).expect("recreate locks directory");

        let second_attempted = hook.join("second-authority-attempted");
        let second_acquired = hook.join("second-acquired");
        let mut second = Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("tests::effect_namespace_subprocess_entrypoint")
            .arg("--nocapture")
            .env("FORGE_EFFECT_NAMESPACE_WORKER_ROOT", &root)
            .env("FORGE_EFFECT_AUTHORITY_ATTEMPTED", &second_attempted)
            .env("FORGE_EFFECT_NAMESPACE_ACQUIRED", &second_acquired)
            .spawn()
            .expect("spawn second effect worker on recreated locks directory");
        wait_for_test_path(&second_attempted);
        std::thread::sleep(Duration::from_millis(250));
        assert!(
            !second_acquired.exists(),
            "replacement-directory worker overlapped durable publication"
        );
        assert!(
            second.try_wait().expect("probe second worker").is_none(),
            "replacement-directory worker did not block on retained state-root authority"
        );
        assert_eq!(
            fs::read(root.join(WAL)).expect("WAL while first worker paused"),
            wal_before,
            "blocked replacement authority must not overlap publication"
        );
        assert!(!root.join("out/committed.txt").exists());

        fs::write(hook.join("release"), b"release").expect("release first worker");
        let first_status = first.wait().expect("wait for first effect worker");
        assert!(
            first_status.success(),
            "first effect worker failed: {first_status}"
        );
        let second_status = second.wait().expect("wait for second effect worker");
        assert!(
            second_status.success(),
            "second effect worker failed: {second_status}"
        );
        assert!(
            second_acquired.exists(),
            "second worker never acquired after first authority dropped"
        );
        assert_eq!(
            fs::read(root.join("out/committed.txt")).expect("committed target"),
            b"single-authority\n"
        );

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[cfg(unix)]
    #[test]
    fn ambient_wal_wrapper_whole_root_swap_publishes_only_to_retained_root() {
        const WAL: &str = ".forge-method/wal/effects.ndjson";
        let root = temp_root("ambient-wrapper-whole-root-swap");
        fs::create_dir_all(root.join(".forge-method")).expect("state root A");
        fs::create_dir_all(root.join("out")).expect("target parent A");
        let hook = root.join("hook");
        fs::create_dir(&hook).expect("hook A");

        let mut worker = Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("tests::effect_namespace_subprocess_entrypoint")
            .arg("--nocapture")
            .env("FORGE_EFFECT_NAMESPACE_WORKER_ROOT", &root)
            .env("FORGE_EFFECT_AMBIENT_WRAPPER", "1")
            .env("FORGE_EFFECT_TEST_PAUSE_AFTER_BEGIN", &hook)
            .spawn()
            .expect("spawn ambient effect worker");
        wait_for_test_path(&hook.join("begin-durable"));

        let retained_a = root.with_extension("retained-authority-a");
        let _ = fs::remove_dir_all(&retained_a);
        fs::rename(&root, &retained_a).expect("displace retained root A");
        fs::create_dir_all(root.join(".forge-method")).expect("replacement state root B");
        fs::create_dir_all(root.join("out")).expect("replacement target parent B");
        fs::create_dir(root.join("hook")).expect("replacement hook B");
        fs::write(root.join("out/committed.txt"), b"replacement-root\n")
            .expect("replacement sentinel");
        fs::write(root.join("hook/release"), b"release").expect("release ambient worker");

        let status = worker.wait().expect("wait for ambient worker");
        assert!(status.success(), "ambient worker failed: {status}");
        assert_eq!(
            fs::read(retained_a.join("out/committed.txt")).expect("retained A target"),
            b"single-authority\n"
        );
        assert_eq!(
            fs::read(root.join("out/committed.txt")).expect("replacement B sentinel"),
            b"replacement-root\n",
            "ambient wrapper crossed into replacement root B"
        );
        assert!(retained_a.join(WAL).exists(), "retained A WAL missing");
        assert!(
            !root.join(WAL).exists(),
            "ambient wrapper published WAL bytes into replacement root B"
        );

        fs::remove_dir_all(root).expect("cleanup replacement root B");
        fs::remove_dir_all(retained_a).expect("cleanup retained root A");
    }

    #[test]
    fn resolve_effect_physical_ref_maps_logical_ids_and_streams() {
        let root = temp_root("resolve-logical-ids");
        let cases = [
            (
                EffectTargetKind::ArtifactId,
                "story-current",
                ".forge-method/artifacts/story-current.yaml",
            ),
            (
                EffectTargetKind::ArtifactId,
                ".forge-method/artifacts/story-current.yaml",
                ".forge-method/artifacts/story-current.yaml",
            ),
            (
                EffectTargetKind::EvidenceId,
                "browser snapshot",
                ".forge-method/evidence/browser_snapshot.json",
            ),
            (
                EffectTargetKind::EvidenceId,
                ".forge-method/snapshots/browser.json",
                ".forge-method/snapshots/browser.json",
            ),
            (
                EffectTargetKind::LedgerStream,
                "agent-main",
                ".forge-method/ledger/agent-main.ndjson",
            ),
            (
                EffectTargetKind::LedgerStream,
                ".forge-method/ledger.ndjson",
                ".forge-method/ledger.ndjson",
            ),
            (
                EffectTargetKind::RequestStream,
                "handoff",
                ".forge-method/requests/handoff.ndjson",
            ),
            (
                EffectTargetKind::RequestStream,
                ".forge-method/requests.ndjson",
                ".forge-method/requests.ndjson",
            ),
        ];

        for (target_kind, reference, expected) in cases {
            let resolved = resolve_effect_physical_ref(&root, target_kind, reference)
                .expect("resolve file-backed target");
            assert_eq!(resolved.0, expected);
        }

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn resolve_effect_physical_ref_rejects_unsupported_and_escaping_refs() {
        let root = temp_root("resolve-invalid");

        let unsupported =
            resolve_effect_physical_ref(&root, EffectTargetKind::StateKey, "runtime.ready")
                .expect_err("state keys are not file-backed effect targets");
        assert!(matches!(
            unsupported,
            EffectTargetResolveError::InvalidTargetPath { .. }
        ));

        let escaping =
            resolve_effect_physical_ref(&root, EffectTargetKind::ArtifactId, "../outside")
                .expect_err("path-like artifact refs must stay under allowed forge dirs");
        assert!(matches!(
            escaping,
            EffectTargetResolveError::InvalidTargetPath { .. }
        ));

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn repo_relative_returns_error_for_path_outside_root() {
        let root = temp_root("repo-relative-outside-root");
        let outside = root
            .parent()
            .expect("temp root has parent")
            .join("forge-core-store-outside-root.yaml");

        let error = repo_relative(&root, &outside).expect_err("outside path should fail closed");

        assert!(matches!(
            error,
            ReferenceIndexBuildError::PathOutsideRoot { .. }
        ));
        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn append_json_line_with_no_sync_writes_record_without_fsync() {
        // ADR-0009: NoSync skips sync_all but the record still lands on disk
        // (write_all + flush). We verify the content is correct and readable;
        // we cannot directly assert that fsync was skipped, but we confirm the
        // API path is reachable and produces the same on-disk bytes as the
        // default SyncOnAppend path.
        let root = temp_root("append-jsonl-no-sync");
        fs::create_dir_all(root.join(".forge-method")).expect("create state root");
        let record = serde_json::json!({"k": "v", "n": 42});

        let path_sync = append_json_line(&root, "log/a.jsonl", &record).expect("sync append");
        let path_nosync =
            append_json_line_with_durability(&root, "log/b.jsonl", &record, WalDurability::NoSync)
                .expect("nosync append");

        let a = fs::read_to_string(&path_sync).expect("read sync file");
        let b = fs::read_to_string(&path_nosync).expect("read nosync file");
        assert_eq!(a, b, "NoSync and SyncOnAppend must write identical bytes");
        assert!(
            a.ends_with('\n'),
            "append_json_line terminates with newline"
        );
        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn generic_append_and_lock_reject_each_eventlog_artifact_and_legacy_alias() {
        let root = temp_root("reserved-eventlog-paths");
        fs::create_dir_all(root.join(".forge-method")).expect("create state root");
        let record = serde_json::json!({"untrusted": true});
        for path in [
            "memory/events.ndjson",
            "research/sources.ndjson",
            "governance/conflicts.ndjson",
            "locks/memory.log.lock",
            "locks/research.sources.lock",
            "locks/governance.conflicts.lock",
        ] {
            for spelling in [path.to_owned(), format!(".forge-method/{path}")] {
                assert!(matches!(
                    append_json_line(&root, &spelling, &record),
                    Err(AppendJsonLineError::ReservedStatePath { .. })
                ));
                assert!(matches!(
                    acquire_effect_store_lock(&root, &spelling),
                    Err(EffectStoreLockError::ReservedStatePath { .. })
                ));
            }
        }
        fs::remove_dir_all(root).expect("cleanup temp root");
    }
    #[test]
    fn wal_durability_default_is_sync_on_append() {
        // The default MUST be the durable variant; this is the load-bearing
        // invariant of ADR-0009 (opt-in only).
        assert_eq!(WalDurability::default(), WalDurability::SyncOnAppend);
    }
}
