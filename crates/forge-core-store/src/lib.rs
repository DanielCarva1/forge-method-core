// The WAL replay, effect application, and metadata-index query entrypoints
// are long because they materialise full typed status structs while walking
// JSON-Lines records line-by-line. Splitting them to satisfy
// `clippy::too_many_lines` would harm readability.
#![allow(clippy::too_many_lines)]

use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::runtime::RuntimeKind;
use forge_core_contracts::tool_effect::{AccessMode, EffectTargetKind, ToolEffectContractDocument};
use forge_core_contracts::{RepoPath, StableId};
use forge_core_trace::TraceEvent;
use forge_core_validate::{
    validate_tool_effect, Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedYamlDocument,
    ReferenceIndex, ReferenceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::instrument;
use yaml_serde::Value;

pub mod claim_wal;
pub mod derive_state;
pub mod replay_anchor;
pub mod replay_wal;

/// The sole authority constructor for claim state. Replays the append-only
/// claims WAL into a typed projection. See [`fn@derive_state`] for the contract.
pub use derive_state::derive_state;

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
    collect_known_paths_recursive(
        root,
        &root
            .join("docs")
            .join("fixtures")
            .join("operation-contract-v0"),
        &mut collection,
    );
    collection
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
    let target = resolve_safe_repo_relative(root, relative_path)?;
    let parent = target
        .parent()
        .ok_or_else(|| AppendJsonLineError::InvalidRelativePath {
            path: relative_path.to_string(),
        })?;
    let mut line = serde_json::to_vec(record).map_err(|source| AppendJsonLineError::Serialize {
        path: target.clone(),
        source: source.to_string(),
    })?;
    line.push(b'\n');

    let _lock = acquire_effect_store_lock(root, lock_relative_path).map_err(|source| {
        AppendJsonLineError::Lock {
            path: lock_relative_path.to_string(),
            source: source.to_string(),
        }
    })?;

    fs::create_dir_all(parent).map_err(|source| AppendJsonLineError::CreateDir {
        path: parent.to_path_buf(),
        source: source.to_string(),
    })?;
    ensure_resolved_parent_within_root(root, &target).map_err(|source| {
        AppendJsonLineError::CreateDir {
            path: parent.to_path_buf(),
            source: source.to_string(),
        }
    })?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&target)
        .map_err(|source| AppendJsonLineError::OpenFile {
            path: target.clone(),
            source: source.to_string(),
        })?;

    file.write_all(&line)
        .map_err(|source| AppendJsonLineError::Write {
            path: target.clone(),
            source: source.to_string(),
        })?;
    file.flush().map_err(|source| AppendJsonLineError::Write {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectTargetResolveError {
    InvalidTargetPath {
        target_kind: EffectTargetKind,
        reference: String,
        source: String,
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
        .map_err(|source| EffectTargetResolveError::InvalidTargetPath {
            target_kind,
            reference: reference.to_string(),
            source: source.to_string(),
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

#[derive(Debug)]
pub struct EffectStoreLock {
    file: File,
    path: PathBuf,
}

impl EffectStoreLock {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for EffectStoreLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectStoreLockError {
    InvalidRelativePath { path: String },
    CreateDir { path: PathBuf, source: String },
    OpenFile { path: PathBuf, source: String },
    Lock { path: PathBuf, source: String },
    WouldBlock { path: PathBuf },
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
    acquire_effect_store_lock_inner(root.as_ref(), lock_relative_path, false)
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
    acquire_effect_store_lock_inner(root.as_ref(), lock_relative_path, true)
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

    let expected_lock = resolve_safe_repo_relative(root, expected_lock_relative_path);
    let lock_matches = expected_lock.is_ok_and(|expected| {
        let expected = expected.canonicalize().unwrap_or(expected);
        let actual = effect_lock
            .path()
            .canonicalize()
            .unwrap_or_else(|_| effect_lock.path().to_path_buf());
        expected == actual
    });
    if !lock_matches {
        reasons.push(EffectApplicationReason::StoreLockFailed);
        diagnostics.push(format!(
            "effect preflight lock scope mismatch: expected {expected_lock_relative_path}, actual {}",
            effect_lock.path().display()
        ));
    }
    if validation_error_count > 0 {
        reasons.push(EffectApplicationReason::EffectValidationErrors);
    }
    validate_file_backed_reads(root, effect, &mut reasons, &mut diagnostics);
    let prepared = prepare_file_writes(root, effect, payloads, &mut reasons, &mut diagnostics);
    let mut metadata_records = Vec::new();
    if reasons.is_empty() {
        if let Some(mut writes) = prepared {
            if revalidate_prepared_writes(root, &mut writes, &mut reasons, &mut diagnostics) {
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

    validate_file_backed_reads(root, effect, &mut reasons, &mut diagnostics);
    let prepared = prepare_file_writes(root, effect, payloads, &mut reasons, &mut diagnostics);

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
    if !revalidate_prepared_writes(root, &mut writes, &mut reasons, &mut diagnostics) {
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }
    let originals = capture_originals(&writes);
    let mut applied_refs = Vec::new();

    for write in &writes {
        if let Err(error) = apply_prepared_write(write) {
            reasons.push(EffectApplicationReason::ApplyFailed);
            diagnostics.push(format!("apply {} failed: {error}", write.target.display()));
            let rollback = rollback_originals(&originals, &mut diagnostics);
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
        Ok(_lock) => apply_file_effect_transaction_with_wal_with_durability(
            root,
            effect,
            payloads,
            wal_relative_path,
            tx_id,
            durability,
        ),
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
/// `durability` threads through to `append_effect_wal_record` for the four
/// WAL appends this function performs (begin, before-image, write-applied,
/// commit). Rollback paths inside this function hard-code [`WalDurability::SyncOnAppend`]
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
    let tx_id = tx_id.into();
    apply_file_effect_transaction_with_wal_inner(
        root.as_ref(),
        effect,
        payloads,
        wal_relative_path,
        &tx_id,
        durability,
        None,
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
    )
}

#[instrument(skip_all, fields(effect_id = %effect.tool_effect_contract.id.0, tx_id = tracing::field::Empty), level = "info")]
fn apply_file_effect_transaction_with_wal_inner(
    root: &Path,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    tx_id: &str,
    durability: WalDurability,
    prepared_authority: Option<(EffectExecutionProvenance, EffectReplayCommitBinding)>,
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
    validate_file_backed_reads(root, effect, &mut reasons, &mut diagnostics);
    let prepared = prepare_file_writes(root, effect, payloads, &mut reasons, &mut diagnostics);
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
    if !revalidate_prepared_writes(root, &mut writes, &mut reasons, &mut diagnostics) {
        return blocked_effect_application_result(
            effect_contract.id.clone(),
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        );
    }
    let originals = capture_originals(&writes);
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
    if append_effect_wal_record(root, wal_relative_path, begin_record, durability).is_err() {
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

    let mut applied_refs = Vec::new();
    for (write, original) in writes.iter().zip(originals.iter()) {
        if append_effect_wal_record(
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
            let rollback = rollback_originals(&originals, &mut diagnostics);
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

        if let Err(error) = apply_prepared_write(write) {
            reasons.push(EffectApplicationReason::ApplyFailed);
            diagnostics.push(format!("apply {} failed: {error}", write.target.display()));
            return rollback_wal_transaction_result(RollbackWalTransaction {
                root,
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

        if let Err(error) = append_effect_wal_record(
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

    if append_effect_wal_record(
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
    let wal_path =
        resolve_reconciliation_relative_path(root, "wal_relative_path", wal_relative_path)?;
    if !wal_path.exists() {
        return Ok(false);
    }
    repair_effect_wal_tail_file(&wal_path)
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
    let wal_path =
        resolve_reconciliation_relative_path(root, "wal_relative_path", wal_relative_path)?;
    if !wal_path.exists() {
        return Ok(Vec::new());
    }
    let _ = repair_effect_wal_tail_file(&wal_path)?;
    let records = read_effect_wal_records_strict(&wal_path)?;
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
    let wal_path =
        resolve_reconciliation_relative_path(root, "wal_relative_path", wal_relative_path)?;
    if !wal_path.exists() {
        return Err(EffectReplayReconciliationError::ConflictingTransaction {
            tx_id: tx_id.to_owned(),
            reason: "effect WAL is missing after replay consume".to_owned(),
        });
    }
    let _ = repair_effect_wal_tail_file(&wal_path)?;
    let records = read_effect_wal_records_strict(&wal_path)?;
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
    append_effect_wal_record(
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
        Ok(_lock) => recover_effect_wal(root, wal_relative_path),
        Err(error) => EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::StoreLockFailed],
            diagnostics: vec![format!("effect store lock failed: {error}")],
        },
    }
}

#[instrument(skip_all, fields(wal_path = %wal_relative_path), level = "info")]
pub fn recover_effect_wal(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
) -> EffectWalRecoveryResult {
    let root = root.as_ref();
    let Ok(wal_path) = resolve_safe_repo_relative(root, wal_relative_path) else {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::RecoveryFailed,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::WalReadFailed],
            diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
        };
    };
    if !wal_path.exists() {
        return EffectWalRecoveryResult {
            status: EffectWalRecoveryStatus::Noop,
            recovered_transactions: Vec::new(),
            reasons: vec![EffectWalRecoveryReason::NoWalFile],
            diagnostics: Vec::new(),
        };
    }

    let text = match fs::read_to_string(&wal_path) {
        Ok(text) => text,
        Err(error) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::WalReadFailed],
                diagnostics: vec![format!("read WAL failed: {error}")],
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
        if rollback_wal_before_images(root, &before_images, &mut diagnostics) {
            recovered_transactions.push(tx_id.clone());
            let effect_id = before_images.first().map_or_else(
                || StableId("unknown_effect".to_string()),
                |record| record.effect_id.clone(),
            );
            let _ = append_effect_wal_record(
                root,
                wal_relative_path,
                EffectWalRecord::stage(&tx_id, effect_id, EffectWalStage::RecoveredRollback),
                // Recovery writes MUST be durable: a crash mid-recovery that
                // loses the RecoveredRollback marker would re-do work on the
                // next reboot. See ADR-0009.
                WalDurability::SyncOnAppend,
            );
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
        Ok(_lock) => rebuild_effect_target_metadata_index_with_durability(
            root,
            wal_relative_path,
            index_relative_path,
            recorded_at,
            durability,
        ),
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
    let Ok(wal_path) = resolve_safe_repo_relative(root, wal_relative_path) else {
        return EffectTargetMetadataIndexRebuildResult {
            status: EffectTargetMetadataIndexRebuildStatus::Failed,
            rebuilt_records: 0,
            appended_records: 0,
            records: Vec::new(),
            reasons: vec![EffectTargetMetadataIndexRebuildReason::WalReadFailed],
            diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
        };
    };
    if !wal_path.exists() {
        return EffectTargetMetadataIndexRebuildResult {
            status: EffectTargetMetadataIndexRebuildStatus::Noop,
            rebuilt_records: 0,
            appended_records: 0,
            records: Vec::new(),
            reasons: vec![EffectTargetMetadataIndexRebuildReason::NoWalFile],
            diagnostics: Vec::new(),
        };
    }

    let text = match fs::read_to_string(&wal_path) {
        Ok(text) => text,
        Err(error) => {
            return EffectTargetMetadataIndexRebuildResult {
                status: EffectTargetMetadataIndexRebuildStatus::Failed,
                rebuilt_records: 0,
                appended_records: 0,
                records: Vec::new(),
                reasons: vec![EffectTargetMetadataIndexRebuildReason::WalReadFailed],
                diagnostics: vec![format!("read WAL failed: {error}")],
            };
        }
    };
    let mut wal_records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        match serde_json::from_str::<EffectWalRecord>(line) {
            Ok(record) => wal_records.push(record),
            Err(error) => {
                return EffectTargetMetadataIndexRebuildResult {
                    status: EffectTargetMetadataIndexRebuildStatus::Failed,
                    rebuilt_records: 0,
                    appended_records: 0,
                    records: Vec::new(),
                    reasons: vec![EffectTargetMetadataIndexRebuildReason::WalParseFailed],
                    diagnostics: vec![format!("parse WAL line {} failed: {error}", index + 1)],
                };
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

    if let Err(error) = append_effect_target_metadata_records_with_durability(
        root,
        index_relative_path,
        &records,
        durability,
    ) {
        return EffectTargetMetadataIndexRebuildResult {
            status: EffectTargetMetadataIndexRebuildStatus::Failed,
            rebuilt_records: records.len(),
            appended_records: 0,
            records,
            reasons: vec![EffectTargetMetadataIndexRebuildReason::MetadataAppendFailed],
            diagnostics: vec![format!("append metadata index failed: {error}")],
        };
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

pub fn query_effect_target_metadata_index(
    root: impl AsRef<Path>,
    index_relative_path: &str,
    query: &EffectTargetMetadataIndexQuery,
) -> EffectTargetMetadataIndexQueryResult {
    let root = root.as_ref();
    let Ok(index_path) = resolve_safe_repo_relative(root, index_relative_path) else {
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
    if !index_path.exists() {
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

    let text = match fs::read_to_string(&index_path) {
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
        Ok(_lock) => compact_effect_wal(root, wal_relative_path),
        Err(error) => EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Failed,
            retained_records: 0,
            dropped_records: 0,
            incomplete_transactions: Vec::new(),
            reasons: vec![EffectWalCompactionReason::StoreLockFailed],
            diagnostics: vec![format!("effect store lock failed: {error}")],
        },
    }
}

pub fn compact_effect_wal(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
) -> EffectWalCompactionResult {
    let root = root.as_ref();
    let Ok(wal_path) = resolve_safe_repo_relative(root, wal_relative_path) else {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Failed,
            retained_records: 0,
            dropped_records: 0,
            incomplete_transactions: Vec::new(),
            reasons: vec![EffectWalCompactionReason::WalReadFailed],
            diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
        };
    };
    let mut compaction_diagnostics = Vec::new();
    recover_effect_wal_compaction_debris(&wal_path, &mut compaction_diagnostics);
    if !wal_path.exists() {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Noop,
            retained_records: 0,
            dropped_records: 0,
            incomplete_transactions: Vec::new(),
            reasons: vec![EffectWalCompactionReason::NoWalFile],
            diagnostics: Vec::new(),
        };
    }

    let records = match read_effect_wal_records(&wal_path) {
        Ok(records) => records,
        Err(reason) => {
            return EffectWalCompactionResult {
                status: EffectWalCompactionStatus::Failed,
                retained_records: 0,
                dropped_records: 0,
                incomplete_transactions: Vec::new(),
                reasons: vec![reason],
                diagnostics: vec![format!("failed to read WAL {}", wal_path.display())],
            };
        }
    };
    let incomplete_transactions = incomplete_wal_transactions(&records);
    let provenance_transactions = provenance_bound_transaction_ids(&records);
    let retained: Vec<_> = records
        .iter()
        .filter(|record| {
            incomplete_transactions.contains(&record.tx_id)
                || provenance_transactions.contains(&record.tx_id)
        })
        .cloned()
        .collect();
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
                return EffectWalCompactionResult {
                    status: EffectWalCompactionStatus::Failed,
                    retained_records: 0,
                    dropped_records: 0,
                    incomplete_transactions,
                    reasons: vec![EffectWalCompactionReason::WalWriteFailed],
                    diagnostics: vec![format!("serialize WAL record failed: {error}")],
                };
            }
        }
    }

    if let Err(error) = write_effect_wal_compaction_manifest(
        &wal_path,
        wal_relative_path,
        "begin",
        retained.len(),
        dropped_records,
        &incomplete_transactions,
    ) {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Failed,
            retained_records: 0,
            dropped_records: 0,
            incomplete_transactions,
            reasons: vec![EffectWalCompactionReason::WalWriteFailed],
            diagnostics: vec![format!("write WAL compaction manifest failed: {error}")],
        };
    }

    if let Err(error) = atomic_replace_file(&wal_path, &content) {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Failed,
            retained_records: 0,
            dropped_records: 0,
            incomplete_transactions,
            reasons: vec![EffectWalCompactionReason::WalWriteFailed],
            diagnostics: vec![format!("write compacted WAL failed: {error}")],
        };
    }

    cleanup_effect_wal_atomic_debris(&wal_path, &mut compaction_diagnostics);
    if let Err(error) = write_effect_wal_compaction_manifest(
        &wal_path,
        wal_relative_path,
        "complete",
        retained.len(),
        dropped_records,
        &incomplete_transactions,
    ) {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Failed,
            retained_records: 0,
            dropped_records: 0,
            incomplete_transactions,
            reasons: vec![EffectWalCompactionReason::WalWriteFailed],
            diagnostics: vec![format!("write WAL compaction manifest failed: {error}")],
        };
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
        diagnostics: compaction_diagnostics,
    }
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
    InvalidRelativePath { path: String },
    CreateDir { path: PathBuf, source: String },
    OpenFile { path: PathBuf, source: String },
    Serialize { path: PathBuf, source: String },
    Write { path: PathBuf, source: String },
    Lock { path: String, source: String },
}

impl fmt::Display for AppendJsonLineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRelativePath { path } => {
                write!(formatter, "invalid repo-relative append path {path}")
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
            Self::CreateDir { path, source } => {
                write!(formatter, "create lock dir {}: {source}", path.display())
            }
            Self::OpenFile { path, source } => {
                write!(formatter, "open lock file {}: {source}", path.display())
            }
            Self::Lock { path, source } => {
                write!(formatter, "lock file {}: {source}", path.display())
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
    let physical_ref = match target_kind {
        EffectTargetKind::FilePath => reference.to_string(),
        EffectTargetKind::ArtifactId => project_logical_target(
            reference,
            &[".forge-method/artifacts/"],
            &[],
            ".forge-method/artifacts",
            ".yaml",
        )?,
        EffectTargetKind::EvidenceId => project_logical_target(
            reference,
            &[".forge-method/evidence/", ".forge-method/snapshots/"],
            &[],
            ".forge-method/evidence",
            ".json",
        )?,
        EffectTargetKind::LedgerStream => project_logical_target(
            reference,
            &[".forge-method/ledger/"],
            &[".forge-method/ledger.ndjson"],
            ".forge-method/ledger",
            ".ndjson",
        )?,
        EffectTargetKind::RequestStream => project_logical_target(
            reference,
            &[".forge-method/requests/"],
            &[".forge-method/requests.ndjson"],
            ".forge-method/requests",
            ".ndjson",
        )?,
        EffectTargetKind::Glob | EffectTargetKind::StateKey | EffectTargetKind::CompletionId => {
            return Err(AppendJsonLineError::InvalidRelativePath {
                path: reference.to_string(),
            });
        }
    };
    let path = resolve_safe_repo_relative(root, &physical_ref)?;
    Ok((path, physical_ref))
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
    existed: bool,
    content: Vec<u8>,
}

fn validate_file_backed_reads(
    root: &Path,
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
        let Ok((path, _physical_ref)) =
            resolve_effect_target(root, read.target_kind, &read.reference)
        else {
            reasons.push(EffectApplicationReason::InvalidTargetPath);
            diagnostics.push(format!("invalid read target path {}", read.reference));
            continue;
        };
        match fs::read(&path) {
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

        let Ok((target, physical_reference)) =
            resolve_effect_target(root, write.target_kind, &write.reference)
        else {
            reasons.push(EffectApplicationReason::InvalidTargetPath);
            diagnostics.push(format!("invalid write target path {}", write.reference));
            continue;
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

        let target_exists = target.exists();
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
            match fs::read(&target) {
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
                let mut content = fs::read(&target).unwrap_or_default();
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

fn revalidate_prepared_writes(
    root: &Path,
    writes: &mut [PreparedWrite],
    reasons: &mut Vec<EffectApplicationReason>,
    diagnostics: &mut Vec<String>,
) -> bool {
    let mut ok = true;
    for write in writes {
        if let Err(error) = ensure_target_chain_within_root(root, &write.target) {
            ok = false;
            reasons.push(EffectApplicationReason::InvalidTargetPath);
            diagnostics.push(format!(
                "target parent escaped repo root {}: {error}",
                write.reference
            ));
            continue;
        }

        let target_exists = write.target.exists();
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
            match fs::read(&write.target) {
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
                let mut content = fs::read(&write.target).unwrap_or_default();
                content.extend_from_slice(payload_content);
                write.content = content;
            }
            PreparedAccessMode::Delete => {
                write.content.clear();
            }
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
    let rollback = rollback_originals(transaction.originals, &mut transaction.diagnostics);
    let rollback_recorded = append_effect_wal_record(
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

fn capture_originals(writes: &[PreparedWrite]) -> Vec<OriginalFileState> {
    writes
        .iter()
        .map(|write| OriginalFileState {
            target: write.target.clone(),
            existed: write.target.exists(),
            content: fs::read(&write.target).unwrap_or_default(),
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

fn apply_prepared_write(write: &PreparedWrite) -> Result<(), ApplyPreparedWriteError> {
    match write.access_mode {
        PreparedAccessMode::Create | PreparedAccessMode::Write | PreparedAccessMode::Append => {
            atomic_replace_file(&write.target, &write.content).map_err(|source| {
                ApplyPreparedWriteError::Io {
                    action: "atomic replace",
                    path: write.target.clone(),
                    source: source.to_string(),
                }
            })
        }
        PreparedAccessMode::Delete => {
            fs::remove_file(&write.target).map_err(|source| ApplyPreparedWriteError::Io {
                action: "remove file",
                path: write.target.clone(),
                source: source.to_string(),
            })?;
            if let Some(parent) = write.target.parent() {
                sync_parent_dir(parent).map_err(|source| ApplyPreparedWriteError::Io {
                    action: "sync parent dir",
                    path: parent.to_path_buf(),
                    source: source.to_string(),
                })?;
            }
            Ok(())
        }
    }
}

fn rollback_originals(originals: &[OriginalFileState], diagnostics: &mut Vec<String>) -> bool {
    let mut ok = true;
    for original in originals.iter().rev() {
        let result = if original.existed {
            atomic_replace_file(&original.target, &original.content)
        } else if original.target.exists() {
            fs::remove_file(&original.target).and_then(|()| {
                if let Some(parent) = original.target.parent() {
                    sync_parent_dir(parent)
                } else {
                    Ok(())
                }
            })
        } else {
            Ok(())
        };
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

fn atomic_replace_file(target: &Path, content: &[u8]) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    fs::create_dir_all(parent)?;

    let nonce = transaction_nonce();
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
    let temp = parent.join(format!(".{file_name}.{nonce}.forge-tmp"));
    let backup = parent.join(format!(".{file_name}.{nonce}.forge-bak"));

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(content)?;
        file.sync_all()?;
    }

    let had_target = target.exists();
    if had_target {
        fs::rename(target, &backup)?;
        sync_parent_dir(parent)?;
    }

    if let Err(error) = fs::rename(&temp, target) {
        let mut cleanup_errors = Vec::new();
        match fs::remove_file(&temp) {
            Ok(()) => {
                if let Err(sync_error) = sync_parent_dir(parent) {
                    cleanup_errors.push(format!(
                        "sync parent after removing temp {} failed: {sync_error}",
                        temp.display()
                    ));
                }
            }
            Err(remove_error) if remove_error.kind() == io::ErrorKind::NotFound => {}
            Err(remove_error) => cleanup_errors.push(format!(
                "remove temp {} failed: {remove_error}",
                temp.display()
            )),
        }
        if had_target {
            match fs::rename(&backup, target) {
                Ok(()) => {
                    if let Err(sync_error) = sync_parent_dir(parent) {
                        cleanup_errors.push(format!(
                            "sync parent after restoring backup {} failed: {sync_error}",
                            backup.display()
                        ));
                    }
                }
                Err(restore_error) => cleanup_errors.push(format!(
                    "restore backup {} to {} failed: {restore_error}",
                    backup.display(),
                    target.display()
                )),
            }
        }
        if cleanup_errors.is_empty() {
            return Err(error);
        }
        return Err(io::Error::new(
            error.kind(),
            format!(
                "rename temp {} to {} failed: {error}; cleanup diagnostics: {}",
                temp.display(),
                target.display(),
                cleanup_errors.join("; ")
            ),
        ));
    }
    sync_parent_dir(parent)?;

    if had_target {
        fs::remove_file(&backup)?;
        sync_parent_dir(parent)?;
    }

    Ok(())
}

#[cfg(unix)]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    if parent.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "parent directory is empty",
        ));
    }
    let _ = File::open(parent).and_then(|file| file.sync_all());
    Ok(())
}

fn transaction_nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

fn validate_effect_lock_scope(
    root: &Path,
    effect_lock: &EffectStoreLock,
    expected_lock_relative_path: &str,
) -> Result<(), EffectReplayReconciliationError> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let expected = resolve_reconciliation_relative_path(
        &canonical_root,
        "expected_lock_relative_path",
        expected_lock_relative_path,
    )?;
    let expected = expected.canonicalize().unwrap_or(expected);
    let actual_path = effect_lock.path().to_path_buf();
    let actual = actual_path.canonicalize().unwrap_or(actual_path);
    if expected != actual || !actual.starts_with(&canonical_root) {
        return Err(EffectReplayReconciliationError::LockScopeMismatch { expected, actual });
    }
    Ok(())
}

fn resolve_reconciliation_relative_path(
    root: &Path,
    field: &'static str,
    relative: &str,
) -> Result<PathBuf, EffectReplayReconciliationError> {
    resolve_safe_repo_relative(root, relative).map_err(|_| {
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

fn repair_effect_wal_tail_file(wal_path: &Path) -> Result<bool, EffectReplayReconciliationError> {
    let bytes = fs::read(wal_path).map_err(|source| EffectReplayReconciliationError::WalRead {
        path: wal_path.to_path_buf(),
        source: source.to_string(),
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
    if complete_final_record {
        let mut file = OpenOptions::new()
            .append(true)
            .open(wal_path)
            .map_err(|source| EffectReplayReconciliationError::WalRepair {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
        file.write_all(b"\n")
            .and_then(|()| file.sync_all())
            .map_err(|source| EffectReplayReconciliationError::WalRepair {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
    } else {
        let file = OpenOptions::new()
            .write(true)
            .open(wal_path)
            .map_err(|source| EffectReplayReconciliationError::WalRepair {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
        file.set_len(u64::try_from(tail_start).unwrap_or(u64::MAX))
            .and_then(|()| file.sync_all())
            .map_err(|source| EffectReplayReconciliationError::WalRepair {
                path: wal_path.to_path_buf(),
                source: source.to_string(),
            })?;
    }
    Ok(true)
}

fn read_effect_wal_records_strict(
    wal_path: &Path,
) -> Result<Vec<EffectWalRecord>, EffectReplayReconciliationError> {
    let content = fs::read_to_string(wal_path).map_err(|source| {
        EffectReplayReconciliationError::WalRead {
            path: wal_path.to_path_buf(),
            source: source.to_string(),
        }
    })?;
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

// Callers pass freshly-built `EffectWalRecord` values, so taking ownership
// keeps the call sites concise without forcing a binding just to take a
// reference.
//
// `durability` threads ADR-0009 through the WAL append path. Recovery and
// rollback callers hard-code `WalDurability::SyncOnAppend` (durability is
// load-bearing there); the apply path threads the caller's choice so
// benchmarks and tests can opt into `NoSync`.
#[allow(clippy::needless_pass_by_value)]
fn append_effect_wal_record(
    root: &Path,
    wal_relative_path: &str,
    record: EffectWalRecord,
    durability: WalDurability,
) -> Result<PathBuf, AppendJsonLineError> {
    append_json_line_with_durability(root, wal_relative_path, &record, durability)
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

fn rollback_wal_before_images(
    root: &Path,
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
        let Ok(target) = resolve_safe_repo_relative(root, physical_target_ref) else {
            diagnostics.push(format!("invalid WAL target path {physical_target_ref}"));
            ok = false;
            continue;
        };
        let result = if original.existed {
            if sha256_content_hash(&original.content) == original.content_hash {
                atomic_replace_file(&target, &original.content)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "WAL before image hash mismatch",
                ))
            }
        } else if target.exists() {
            fs::remove_file(&target).and_then(|()| {
                if let Some(parent) = target.parent() {
                    sync_parent_dir(parent)
                } else {
                    Ok(())
                }
            })
        } else {
            Ok(())
        };
        if let Err(error) = result {
            diagnostics.push(format!("WAL rollback {} failed: {error}", target.display()));
            ok = false;
        }
    }
    ok
}

fn acquire_effect_store_lock_inner(
    root: &Path,
    lock_relative_path: &str,
    try_only: bool,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    let path = resolve_safe_repo_relative(root, lock_relative_path).map_err(|_| {
        EffectStoreLockError::InvalidRelativePath {
            path: lock_relative_path.to_string(),
        }
    })?;
    let parent = path
        .parent()
        .ok_or_else(|| EffectStoreLockError::InvalidRelativePath {
            path: lock_relative_path.to_string(),
        })?;
    fs::create_dir_all(parent).map_err(|source| EffectStoreLockError::CreateDir {
        path: parent.to_path_buf(),
        source: source.to_string(),
    })?;
    ensure_resolved_parent_within_root(root, &path).map_err(|source| {
        EffectStoreLockError::CreateDir {
            path: parent.to_path_buf(),
            source: source.to_string(),
        }
    })?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| EffectStoreLockError::OpenFile {
            path: path.clone(),
            source: source.to_string(),
        })?;
    if try_only {
        match file.try_lock() {
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
    Ok(EffectStoreLock { file, path })
}

const EFFECT_STORE_LOCK_RETRY_ATTEMPTS: u32 = 60;

fn acquire_effect_store_lock_with_deadline(
    file: &File,
    path: &Path,
) -> Result<(), EffectStoreLockError> {
    for attempt in 0..EFFECT_STORE_LOCK_RETRY_ATTEMPTS {
        match file.try_lock() {
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

fn effect_wal_compaction_manifest_path(wal_path: &Path) -> PathBuf {
    let parent = wal_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = wal_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("wal");
    parent.join(format!(".{file_name}.compaction-manifest.json"))
}

fn write_effect_wal_compaction_manifest(
    wal_path: &Path,
    wal_relative_path: &str,
    status: &str,
    retained_records: usize,
    dropped_records: usize,
    incomplete_transactions: &[String],
) -> io::Result<()> {
    let manifest = EffectWalCompactionManifest {
        schema_version: "0.1".to_string(),
        wal_relative_path: wal_relative_path.to_string(),
        status: status.to_string(),
        retained_records,
        dropped_records,
        incomplete_transactions: incomplete_transactions.to_vec(),
    };
    let content = serde_json::to_vec(&manifest)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let manifest_path = effect_wal_compaction_manifest_path(wal_path);
    atomic_replace_file(&manifest_path, &content)
}

fn recover_effect_wal_compaction_debris(wal_path: &Path, diagnostics: &mut Vec<String>) {
    let manifest_path = effect_wal_compaction_manifest_path(wal_path);
    if !manifest_path.exists() {
        return;
    }
    if !wal_path.exists() {
        restore_latest_effect_wal_backup(wal_path, diagnostics);
    }
    cleanup_effect_wal_atomic_debris(wal_path, diagnostics);
}

fn restore_latest_effect_wal_backup(wal_path: &Path, diagnostics: &mut Vec<String>) {
    let Some(parent) = wal_path.parent() else {
        return;
    };
    let Some(file_name) = wal_path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let prefix = format!(".{file_name}.");
    let mut backups = Vec::new();
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".forge-bak") {
            backups.push(entry.path());
        }
    }
    backups.sort();
    if let Some(backup) = backups.pop() {
        match fs::rename(&backup, wal_path) {
            Ok(()) => {
                let _ = sync_parent_dir(parent);
                diagnostics.push(format!(
                    "restored WAL backup {} after interrupted compaction",
                    backup.display()
                ));
            }
            Err(error) => diagnostics.push(format!(
                "restore WAL backup {} failed: {error}",
                backup.display()
            )),
        }
    }
}

fn cleanup_effect_wal_atomic_debris(wal_path: &Path, diagnostics: &mut Vec<String>) {
    let Some(parent) = wal_path.parent() else {
        return;
    };
    let Some(file_name) = wal_path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    let prefix = format!(".{file_name}.");
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let mut removed_any = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix)
            && (name.ends_with(".forge-tmp") || name.ends_with(".forge-bak"))
        {
            match fs::remove_file(entry.path()) {
                Ok(()) => {
                    removed_any = true;
                    diagnostics.push(format!("removed WAL compaction debris {name}"));
                }
                Err(error) => diagnostics.push(format!(
                    "remove WAL compaction debris {name} failed: {error}"
                )),
            }
        }
    }
    if removed_any {
        let _ = sync_parent_dir(parent);
    }
}

fn read_effect_wal_records(
    wal_path: &Path,
) -> Result<Vec<EffectWalRecord>, EffectWalCompactionReason> {
    let text =
        fs::read_to_string(wal_path).map_err(|_| EffectWalCompactionReason::WalReadFailed)?;
    text.lines()
        .map(|line| {
            serde_json::from_str::<EffectWalRecord>(line)
                .map_err(|_| EffectWalCompactionReason::WalParseFailed)
        })
        .collect()
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
    use std::time::{SystemTime, UNIX_EPOCH};

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
    fn wal_durability_default_is_sync_on_append() {
        // The default MUST be the durable variant; this is the load-bearing
        // invariant of ADR-0009 (opt-in only).
        assert_eq!(WalDurability::default(), WalDurability::SyncOnAppend);
    }
}
