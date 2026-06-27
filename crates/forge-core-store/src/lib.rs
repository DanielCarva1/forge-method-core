use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::runtime::RuntimeKind;
use forge_core_contracts::tool_effect::{AccessMode, EffectTargetKind, ToolEffectContractDocument};
use forge_core_contracts::StableId;
use forge_core_validate::{
    validate_tool_effect, Diagnostic, DiagnosticCode, DiagnosticSeverity, ParsedYamlDocument,
    ReferenceIndex, ReferenceKind,
};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CONTRACT_DEFINITIONS: &[&str] = &[
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
];

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
}

impl ReferenceIndexBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_options(options: ReferenceIndexOptions) -> Self {
        Self { options }
    }

    pub fn build(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<ReferenceIndex, ReferenceIndexBuildError> {
        let root = root.as_ref();
        let mut index = ReferenceIndex::new();

        add_contract_definitions(&mut index, root);
        add_policy_files(&mut index, root)?;
        add_operation_fixtures(&mut index, root)?;
        add_contract_instances(&mut index, root)?;
        add_command_contracts(&mut index, root)?;
        add_runtime_contracts(&mut index, root)?;
        add_runtime_state_refs(&mut index, root, &self.options);

        Ok(index)
    }
}

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

pub fn append_json_line<T>(
    root: impl AsRef<Path>,
    relative_path: &str,
    record: &T,
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
        source,
    })?;
    line.push(b'\n');

    let lock_relative_path = append_json_line_lock_relative_path(relative_path);
    let _lock = acquire_effect_store_lock(root, &lock_relative_path).map_err(|source| {
        AppendJsonLineError::Lock {
            path: lock_relative_path,
            source: source.to_string(),
        }
    })?;

    fs::create_dir_all(parent).map_err(|source| AppendJsonLineError::CreateDir {
        path: parent.to_path_buf(),
        source,
    })?;
    ensure_resolved_parent_within_root(root, &target).map_err(|source| {
        AppendJsonLineError::CreateDir {
            path: parent.to_path_buf(),
            source,
        }
    })?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&target)
        .map_err(|source| AppendJsonLineError::OpenFile {
            path: target.clone(),
            source,
        })?;

    file.write_all(&line)
        .map_err(|source| AppendJsonLineError::Write {
            path: target.clone(),
            source,
        })?;
    file.flush().map_err(|source| AppendJsonLineError::Write {
        path: target.clone(),
        source,
    })?;
    file.sync_all()
        .map_err(|source| AppendJsonLineError::Write {
            path: target.clone(),
            source,
        })?;

    Ok(target)
}

pub fn append_effect_target_metadata_records(
    root: impl AsRef<Path>,
    index_relative_path: &str,
    records: &[EffectTargetMetadataRecord],
) -> Result<Vec<PathBuf>, AppendJsonLineError> {
    let root = root.as_ref();
    let mut paths = Vec::with_capacity(records.len());
    for record in records {
        paths.push(append_json_line(root, index_relative_path, record)?);
    }
    Ok(paths)
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
    StoreLockFailed,
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
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for EffectStoreLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug)]
pub enum EffectStoreLockError {
    InvalidRelativePath { path: String },
    CreateDir { path: PathBuf, source: io::Error },
    OpenFile { path: PathBuf, source: io::Error },
    Lock { path: PathBuf, source: io::Error },
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

pub fn sha256_content_hash(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    format!("sha256:{digest:x}")
}

pub fn acquire_effect_store_lock(
    root: impl AsRef<Path>,
    lock_relative_path: &str,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    acquire_effect_store_lock_inner(root.as_ref(), lock_relative_path, false)
}

pub fn try_acquire_effect_store_lock(
    root: impl AsRef<Path>,
    lock_relative_path: &str,
) -> Result<EffectStoreLock, EffectStoreLockError> {
    acquire_effect_store_lock_inner(root.as_ref(), lock_relative_path, true)
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
        return EffectApplicationResult {
            status: EffectApplicationStatus::Blocked,
            effect_id: effect_contract.id.clone(),
            applied_refs: Vec::new(),
            metadata_records: Vec::new(),
            rolled_back: false,
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        };
    }

    let mut writes = prepared.expect("prepared writes when no preflight reasons");
    if !revalidate_prepared_writes(root, &mut writes, &mut reasons, &mut diagnostics) {
        return EffectApplicationResult {
            status: EffectApplicationStatus::Blocked,
            effect_id: effect_contract.id.clone(),
            applied_refs: Vec::new(),
            metadata_records: Vec::new(),
            rolled_back: false,
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        };
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
    let root = root.as_ref();
    match acquire_effect_store_lock(root, lock_relative_path) {
        Ok(_lock) => {
            apply_file_effect_transaction_with_wal(root, effect, payloads, wal_relative_path, tx_id)
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

pub fn apply_file_effect_transaction_with_wal(
    root: impl AsRef<Path>,
    effect: &ToolEffectContractDocument,
    payloads: &[EffectApplicationPayload],
    wal_relative_path: &str,
    tx_id: impl Into<String>,
) -> EffectApplicationResult {
    let root = root.as_ref();
    let tx_id = tx_id.into();
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
        return EffectApplicationResult {
            status: EffectApplicationStatus::Blocked,
            effect_id: effect_contract.id.clone(),
            applied_refs: Vec::new(),
            metadata_records: Vec::new(),
            rolled_back: false,
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        };
    }

    let mut writes = prepared.expect("prepared writes when no preflight reasons");
    if !revalidate_prepared_writes(root, &mut writes, &mut reasons, &mut diagnostics) {
        return EffectApplicationResult {
            status: EffectApplicationStatus::Blocked,
            effect_id: effect_contract.id.clone(),
            applied_refs: Vec::new(),
            metadata_records: Vec::new(),
            rolled_back: false,
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        };
    }
    let originals = capture_originals(&writes);
    if append_effect_wal_record(
        root,
        wal_relative_path,
        EffectWalRecord::begin(&tx_id, effect_contract.id.clone()),
    )
    .is_err()
    {
        reasons.push(EffectApplicationReason::WalAppendFailed);
        diagnostics.push("failed to append WAL begin record".to_string());
        return EffectApplicationResult {
            status: EffectApplicationStatus::Blocked,
            effect_id: effect_contract.id.clone(),
            applied_refs: Vec::new(),
            metadata_records: Vec::new(),
            rolled_back: false,
            reasons,
            diagnostics,
            validation_error_count,
            validation_warning_count,
        };
    }

    let mut applied_refs = Vec::new();
    for (write, original) in writes.iter().zip(originals.iter()) {
        if append_effect_wal_record(
            root,
            wal_relative_path,
            EffectWalRecord::before_image(&tx_id, effect_contract.id.clone(), write, original),
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
                tx_id: &tx_id,
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
            EffectWalRecord::write_applied(&tx_id, effect, write),
        ) {
            reasons.push(EffectApplicationReason::WalAppendFailed);
            diagnostics.push(format!(
                "failed to append WAL write-applied {}: {error}",
                write.reference
            ));
            return rollback_wal_transaction_result(RollbackWalTransaction {
                root,
                wal_relative_path,
                tx_id: &tx_id,
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
        EffectWalRecord::stage(&tx_id, effect_contract.id.clone(), EffectWalStage::Commit),
    )
    .is_err()
    {
        reasons.push(EffectApplicationReason::WalAppendFailed);
        diagnostics.push("failed to append WAL commit record".to_string());
        return rollback_wal_transaction_result(RollbackWalTransaction {
            root,
            wal_relative_path,
            tx_id: &tx_id,
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

pub fn recover_effect_wal(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
) -> EffectWalRecoveryResult {
    let root = root.as_ref();
    let wal_path = match resolve_safe_repo_relative(root, wal_relative_path) {
        Ok(path) => path,
        Err(_) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::WalReadFailed],
                diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
            };
        }
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
        Err(diagnostic) => {
            return EffectWalRecoveryResult {
                status: EffectWalRecoveryStatus::RecoveryFailed,
                recovered_transactions: Vec::new(),
                reasons: vec![EffectWalRecoveryReason::WalParseFailed],
                diagnostics: vec![diagnostic],
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
            let effect_id = before_images
                .first()
                .map(|record| record.effect_id.clone())
                .unwrap_or_else(|| StableId("unknown_effect".to_string()));
            let _ = append_effect_wal_record(
                root,
                wal_relative_path,
                EffectWalRecord::stage(&tx_id, effect_id, EffectWalStage::RecoveredRollback),
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
) -> Result<(Vec<EffectWalRecord>, Vec<String>), String> {
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
                return Err(format!("parse WAL line {} failed: {error}", index + 1));
            }
        }
    }

    Ok((records, diagnostics))
}

pub fn rebuild_effect_target_metadata_index_with_lock(
    root: impl AsRef<Path>,
    wal_relative_path: &str,
    index_relative_path: &str,
    lock_relative_path: &str,
    recorded_at: Option<&str>,
) -> EffectTargetMetadataIndexRebuildResult {
    let root = root.as_ref();
    match acquire_effect_store_lock(root, lock_relative_path) {
        Ok(_lock) => rebuild_effect_target_metadata_index(
            root,
            wal_relative_path,
            index_relative_path,
            recorded_at,
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
    let root = root.as_ref();
    let wal_path = match resolve_safe_repo_relative(root, wal_relative_path) {
        Ok(path) => path,
        Err(_) => {
            return EffectTargetMetadataIndexRebuildResult {
                status: EffectTargetMetadataIndexRebuildStatus::Failed,
                rebuilt_records: 0,
                appended_records: 0,
                records: Vec::new(),
                reasons: vec![EffectTargetMetadataIndexRebuildReason::WalReadFailed],
                diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
            };
        }
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

    if let Err(error) = append_effect_target_metadata_records(root, index_relative_path, &records) {
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
    let index_path = match resolve_safe_repo_relative(root, index_relative_path) {
        Ok(path) => path,
        Err(_) => {
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
        }
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
    let wal_path = match resolve_safe_repo_relative(root, wal_relative_path) {
        Ok(path) => path,
        Err(_) => {
            return EffectWalCompactionResult {
                status: EffectWalCompactionStatus::Failed,
                retained_records: 0,
                dropped_records: 0,
                incomplete_transactions: Vec::new(),
                reasons: vec![EffectWalCompactionReason::WalReadFailed],
                diagnostics: vec![format!("invalid WAL path {wal_relative_path}")],
            };
        }
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
    let retained: Vec<_> = records
        .iter()
        .filter(|record| incomplete_transactions.contains(&record.tx_id))
        .cloned()
        .collect();
    let dropped_records = records.len().saturating_sub(retained.len());
    if dropped_records == 0 {
        return EffectWalCompactionResult {
            status: EffectWalCompactionStatus::Noop,
            retained_records: records.len(),
            dropped_records: 0,
            incomplete_transactions,
            reasons: vec![EffectWalCompactionReason::NoClosedRecords],
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

    let reasons = vec![EffectWalCompactionReason::ClosedRecordsDropped];
    EffectWalCompactionResult {
        status: EffectWalCompactionStatus::Compacted,
        retained_records: retained.len(),
        dropped_records,
        incomplete_transactions,
        reasons,
        diagnostics: compaction_diagnostics,
    }
}

#[derive(Debug)]
pub enum ReferenceIndexBuildError {
    ReadDir {
        path: PathBuf,
        source: io::Error,
    },
    ReadFile {
        path: PathBuf,
        source: io::Error,
    },
    ParseYaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
}

impl fmt::Display for ReferenceIndexBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

#[derive(Debug)]
pub enum AppendJsonLineError {
    InvalidRelativePath {
        path: String,
    },
    CreateDir {
        path: PathBuf,
        source: io::Error,
    },
    OpenFile {
        path: PathBuf,
        source: io::Error,
    },
    Serialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    Write {
        path: PathBuf,
        source: io::Error,
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

fn add_contract_definitions(index: &mut ReferenceIndex, root: &Path) {
    for reference in CONTRACT_DEFINITIONS {
        insert_existing(index, root, reference, ReferenceKind::ContractDefinition);
    }
}

fn add_policy_files(
    index: &mut ReferenceIndex,
    root: &Path,
) -> Result<(), ReferenceIndexBuildError> {
    add_yaml_files_in_dir(index, root, "contracts/policies", ReferenceKind::Policy)?;
    insert_existing(
        index,
        root,
        "contracts/operations/operation-reference-policy-v0.yaml",
        ReferenceKind::Policy,
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
        index.insert(repo_relative(root, &path), ReferenceKind::CommandContract);
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
            index.insert(repo_relative(root, &path), kind);
        }
    }
    Ok(())
}

fn add_runtime_state_refs(
    index: &mut ReferenceIndex,
    root: &Path,
    options: &ReferenceIndexOptions,
) {
    insert_existing(
        index,
        root,
        ".forge-method/ledger.ndjson",
        ReferenceKind::Ledger,
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
            index.insert(repo_relative(root, &path), kind);
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
        index.insert(repo_relative(root, &path), kind);
    }
    Ok(())
}

fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>, ReferenceIndexBuildError> {
    let entries = fs::read_dir(dir).map_err(|source| ReferenceIndexBuildError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| ReferenceIndexBuildError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
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
            collection.diagnostics.push(Diagnostic::error(
                DiagnosticCode::YamlReadFailed,
                repo_relative(root, dir),
                source.to_string(),
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                collection.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::YamlReadFailed,
                    repo_relative(root, dir),
                    source.to_string(),
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_documents_recursive(root, &path, collection);
        } else if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
            match read_yaml_value(&path) {
                Ok(value) => collection.documents.push(ParsedYamlDocument {
                    path: repo_relative(root, &path),
                    value,
                }),
                Err(source) => collection.diagnostics.push(Diagnostic::error(
                    DiagnosticCode::YamlParseFailed,
                    repo_relative(root, &path),
                    source.to_string(),
                )),
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
    collection.paths.insert(repo_relative(root, path));
    if !path.is_dir() {
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(source) => {
            collection.diagnostics.push(Diagnostic::error(
                DiagnosticCode::YamlReadFailed,
                repo_relative(root, path),
                source.to_string(),
            ));
            return;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => collect_known_paths_recursive(root, &entry.path(), collection),
            Err(source) => collection.diagnostics.push(Diagnostic::error(
                DiagnosticCode::YamlReadFailed,
                repo_relative(root, path),
                source.to_string(),
            )),
        }
    }
}

fn read_yaml_value(path: &Path) -> Result<Value, ReferenceIndexBuildError> {
    let text = fs::read_to_string(path).map_err(|source| ReferenceIndexBuildError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    serde_yaml::from_str(&text).map_err(|source| ReferenceIndexBuildError::ParseYaml {
        path: path.to_path_buf(),
        source,
    })
}

fn insert_existing(index: &mut ReferenceIndex, root: &Path, reference: &str, kind: ReferenceKind) {
    if root.join(reference).exists() {
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
            .map(|canonical_parent| canonical_parent.starts_with(canonical_root))
            .unwrap_or(false);
    }

    let mut ancestor = parent;
    while !ancestor.exists() {
        let Some(next) = ancestor.parent() else {
            return false;
        };
        ancestor = next;
    }
    fs::canonicalize(ancestor)
        .map(|canonical_ancestor| {
            canonical_ancestor.starts_with(canonical_root) || resolved.starts_with(canonical_root)
        })
        .unwrap_or(false)
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
    access_mode: AccessMode,
    destructive: bool,
    expected_hash: Option<String>,
    payload_content: Option<Vec<u8>>,
    content: Vec<u8>,
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
        if write.access_mode == AccessMode::Read {
            reasons.push(EffectApplicationReason::UnsupportedAccessMode);
            diagnostics.push(format!(
                "unsupported write access mode for {}",
                write.reference
            ));
            continue;
        }

        let (target, physical_reference) =
            match resolve_effect_target(root, write.target_kind, &write.reference) {
                Ok(resolved) => resolved,
                Err(_) => {
                    reasons.push(EffectApplicationReason::InvalidTargetPath);
                    diagnostics.push(format!("invalid write target path {}", write.reference));
                    continue;
                }
            };

        let payload = if write.access_mode == AccessMode::Delete {
            None
        } else {
            match payload_for(payloads, &write.reference) {
                Some(payload) => Some(payload),
                None => {
                    reasons.push(EffectApplicationReason::MissingPayloadForWrite);
                    diagnostics.push(format!("missing payload for {}", write.reference));
                    continue;
                }
            }
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
        match write.access_mode {
            AccessMode::Create if target_exists => {
                reasons.push(EffectApplicationReason::TargetExistsForCreate);
                diagnostics.push(format!("create target exists {}", write.reference));
                continue;
            }
            AccessMode::Write if !target_exists => {
                reasons.push(EffectApplicationReason::TargetMissingForWrite);
                diagnostics.push(format!("write target missing {}", write.reference));
                continue;
            }
            AccessMode::Write if write.expected_hash.is_none() => {
                reasons.push(EffectApplicationReason::MissingExpectedHashForOverwrite);
                diagnostics.push(format!(
                    "write target missing expected hash {}",
                    write.reference
                ));
                continue;
            }
            AccessMode::Delete if !target_exists => {
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

        let content = match (write.access_mode, payload) {
            (AccessMode::Create | AccessMode::Write, Some(payload)) => payload.content.clone(),
            (AccessMode::Append, Some(payload)) => {
                let mut content = fs::read(&target).unwrap_or_default();
                content.extend_from_slice(&payload.content);
                content
            }
            (AccessMode::Delete, _) => Vec::new(),
            (AccessMode::Read, _) => unreachable!("read access mode was rejected"),
            (_, None) => unreachable!("non-delete payload absence was rejected"),
        };

        writes.push(PreparedWrite {
            reference: write.reference.clone(),
            physical_reference,
            target,
            target_kind: write.target_kind,
            access_mode: write.access_mode,
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
            AccessMode::Create if target_exists => {
                ok = false;
                reasons.push(EffectApplicationReason::TargetExistsForCreate);
                diagnostics.push(format!("create target exists {}", write.reference));
                continue;
            }
            AccessMode::Write if !target_exists => {
                ok = false;
                reasons.push(EffectApplicationReason::TargetMissingForWrite);
                diagnostics.push(format!("write target missing {}", write.reference));
                continue;
            }
            AccessMode::Delete if !target_exists => {
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
            AccessMode::Create | AccessMode::Write => {
                write.content = write.payload_content.clone().unwrap_or_default();
            }
            AccessMode::Append => {
                let mut content = fs::read(&write.target).unwrap_or_default();
                if let Some(payload_content) = &write.payload_content {
                    content.extend_from_slice(payload_content);
                }
                write.content = content;
            }
            AccessMode::Delete => {
                write.content.clear();
            }
            AccessMode::Read => unreachable!("read access mode was rejected"),
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
            access_mode: write.access_mode,
            content_hash: if write.access_mode == AccessMode::Delete {
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
        .map(|index| records[index].clone())
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

fn apply_prepared_write(write: &PreparedWrite) -> io::Result<()> {
    match write.access_mode {
        AccessMode::Create | AccessMode::Write | AccessMode::Append => {
            atomic_replace_file(&write.target, &write.content)
        }
        AccessMode::Delete => {
            fs::remove_file(&write.target)?;
            if let Some(parent) = write.target.parent() {
                sync_parent_dir(parent)?;
            }
            Ok(())
        }
        AccessMode::Read => unreachable!("read access mode was rejected"),
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
        if fs::remove_file(&temp).is_ok() {
            sync_parent_dir(parent)?;
        }
        if had_target && fs::rename(&backup, target).is_ok() {
            sync_parent_dir(parent)?;
        }
        return Err(error);
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
        }
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
                access_mode: write.access_mode,
                content_hash: if write.access_mode == AccessMode::Delete {
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
        }
    }
}

fn append_effect_wal_record(
    root: &Path,
    wal_relative_path: &str,
    record: EffectWalRecord,
) -> Result<PathBuf, AppendJsonLineError> {
    append_json_line(root, wal_relative_path, &record)
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
        let target = match resolve_safe_repo_relative(root, physical_target_ref) {
            Ok(target) => target,
            Err(_) => {
                diagnostics.push(format!("invalid WAL target path {physical_target_ref}"));
                ok = false;
                continue;
            }
        };
        let result = if original.existed {
            if sha256_content_hash(&original.content) != original.content_hash {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "WAL before image hash mismatch",
                ))
            } else {
                atomic_replace_file(&target, &original.content)
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
        source,
    })?;
    ensure_resolved_parent_within_root(root, &path).map_err(|source| {
        EffectStoreLockError::CreateDir {
            path: parent.to_path_buf(),
            source,
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
            source,
        })?;
    if try_only {
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(EffectStoreLockError::WouldBlock { path });
            }
            Err(TryLockError::Error(source)) => {
                return Err(EffectStoreLockError::Lock { path, source });
            }
        }
    } else {
        file.lock().map_err(|source| EffectStoreLockError::Lock {
            path: path.clone(),
            source,
        })?;
    }
    Ok(EffectStoreLock { file, path })
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

fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("path under root")
        .to_string_lossy()
        .replace('\\', "/")
}
