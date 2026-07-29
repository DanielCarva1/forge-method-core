//! Governed isolated-work preview and exact local-reversible apply.
//!
//! This module binds one Active isolation contract to retained source and
//! destination trees plus current objective, evidence, ledger, and claim-WAL
//! projections. Apply re-derives the same facts under retained locks, records a
//! durable replay intent, writes only exact existing regular files, reads back
//! canonical state, and publishes a self-digested receipt.

use super::adapter::{WorkflowGovernanceGuidance, WorkflowGovernanceProjectBinding};
use forge_core_contracts::claim::ActorRole;
use forge_core_contracts::isolation::{
    IsolationContract, IsolationContractDocument, IsolationStatus,
};
use forge_core_contracts::tool_effect::{
    AccessMode, ConflictCode, ConflictDetection, ConflictPolicy, EffectActor, EffectKind,
    EffectNotification, EffectRead, EffectRepair, EffectTargetKind, EffectWrite, InverseKind,
    InverseMetadata, InverseSource, RepairStrategy,
};
use forge_core_contracts::{
    ClaimId, GovernedPromotionApplication, GovernedPromotionApplyStatus, GovernedPromotionPreview,
    GovernedPromotionPreviewAuthority, GovernedPromotionReceipt, PrincipalId,
    PromotionAppliedFileBinding, PromotionApplyEligibility, PromotionAssuranceClaimCoverage,
    PromotionAssuranceClaimStatus, PromotionCarriedAssuranceGap, PromotionClaimConflict,
    PromotionClaimSetBinding, PromotionDestinationBinding, PromotionDiffEffect,
    PromotionEvidenceRecordBinding, PromotionEvidenceSetBinding, PromotionExcludedRootBinding,
    PromotionExcludedRootKind, PromotionGitWorktreeBinding, PromotionGovernanceBinding,
    PromotionObjectiveBinding, PromotionObjectiveCoverage, PromotionObjectiveCoverageStatus,
    PromotionPathClaimAttribution, PromotionRecoveryExecutionBinding, PromotionReplayBinding,
    PromotionSnapshotBinding, PromotionSourceBinding, PromotionUnsupportedEffect,
    PromotionUnsupportedEffectKind, PromotionWriteClaimCoverage, PromotionWriteClaimCoverageStatus,
    RepoPath, StableId, ToolEffectContract, ToolEffectContractDocument,
    WorkflowCooperativeEvidenceCurrentStatus, WorkflowCooperativeEvidenceDisposition,
    WorkflowReadinessProfile, GOVERNED_PROMOTION_PREVIEW_SCHEMA_VERSION,
    GOVERNED_PROMOTION_RECEIPT_SCHEMA_VERSION,
};
use forge_core_decisions::{
    check_write_against_claims, derive_promotion_diff, detect_isolation_conflict,
    evaluate_promotion_readiness, is_live, promotion_domain_digest, rfc3339_to_unix,
    validate_isolation_contract, PromotionInventoryDirectory, PromotionInventoryFile,
    PromotionPlanningError, PromotionReadinessInput, WorkflowClaimResultStatus, WriteCheck,
};
use forge_core_store::claim_wal::{
    project_existing_claim_wal, ClaimWalProjection, ClaimWalProjectionOptions,
    ClaimWalProjectionStopPolicy, CLAIM_WAL_LOCK_RELATIVE_PATH, CLAIM_WAL_RELATIVE_PATH,
};
use forge_core_store::replay_wal::{
    acquire_replay_commit_guard, consume_replay_key_hash_under_effect_lock,
    inspect_replay_key_hash_under_effect_lock, replay_nonce_key_hash,
    reserve_replay_nonce_under_effect_lock, verify_consumed_replay_key_hash_under_effect_lock,
    ReplayReservationState, ReplayWalError,
};
use forge_core_store::retained_project_tree::{RetainedProjectTree, RetainedProjectTreeError};
use forge_core_store::{
    acquire_existing_effect_store_lock, append_effect_replay_completion_under_lock,
    apply_existing_file_effect_transaction_to_retained_project_tree,
    inspect_split_root_effect_transaction_under_lock,
    recover_existing_file_effect_transaction_to_retained_project_tree, sha256_content_hash,
    EffectApplicationPayload, EffectApplicationStatus, EffectExecutionProvenance,
    EffectReplayCommitBinding, EffectStoreLock, SplitRootEffectTransactionInspection,
    SplitRootEffectTransactionStage,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

const MAX_PROMOTION_SNAPSHOT_ENTRIES: usize = 200_000;
const MAX_PROMOTION_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ISOLATION_DOCUMENTS: usize = 4_096;
const MAX_ISOLATION_DOCUMENT_BYTES: u64 = 1024 * 1024;
const MAX_ISOLATION_DOCUMENT_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_PROMOTION_STATE_DOCUMENTS: usize = 4_096;
const MAX_PROMOTION_STATE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
const EXCLUDED_ROOT_NAMES: &[&str] = &[".git", ".forge-method", "target", "node_modules"];
pub(super) const PROMOTION_EFFECT_LOCK_RELATIVE_PATH: &str = "promotion/apply.lock";
const PROMOTION_EFFECT_WAL_RELATIVE_PATH: &str = "promotion/effects.ndjson";
const PROMOTION_RECEIPT_MAX_BYTES: u64 = 64 * 1024 * 1024;
const PROMOTION_REPLAY_AUDIENCE: &str = "forge.workflow.promotion.apply.local-reversible.v1";
const PROMOTION_RECOVERY_EXECUTION_SCHEMA_VERSION: &str =
    "governed_promotion_recovery_execution_v1";
const PROMOTION_PRE_BEGIN_RECOVERY_KIND: &str = "pre_begin_fresh_execution_v1";
const PROMOTION_LEGACY_V1_PRE_BEGIN_RECOVERY_KIND: &str = "legacy_v1_pre_begin_fresh_execution_v1";

#[derive(Debug)]
#[non_exhaustive]
pub enum PromotionPreviewError {
    SoloProfileRequired,
    ActiveObjectiveRequired,
    IsolationDirectory { path: PathBuf, source: String },
    IsolationDocument { path: PathBuf, source: String },
    TooManyIsolationDocuments,
    IsolationNotFound(String),
    DuplicateIsolationId(String),
    IsolationInvalid(String),
    IsolationNotActive(String),
    SourceRoot { path: PathBuf, source: String },
    SourceAliasesDestination,
    NonUtf8Path(PathBuf),
    Snapshot(RetainedProjectTreeError),
    SnapshotBindingMismatch,
    ClaimProjection(String),
    ClaimProjectionChanged,
    Planning(PromotionPlanningError),
    IsolationChanged,
    ClockOverflow,
    GitWorktree(String),
    FreshnessExpiredDuringDerivation,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PromotionApplyError {
    InvalidExpectedPreviewDigest,
    Store(String),
    ReceiptInvalid(String),
    RecoveryRequired(String),
    Preview(PromotionPreviewError),
    PreviewDigestMismatch { expected: String, actual: String },
    NotEligible(String),
    MissingDerivedPrincipal,
    UnsupportedEffect(String),
    Payload(String),
    Effect(String),
    Readback(String),
}

impl std::fmt::Display for PromotionApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidExpectedPreviewDigest => {
                formatter.write_str("expected preview digest must be one sha256 digest")
            }
            Self::Store(reason) => write!(formatter, "promotion store failed: {reason}"),
            Self::ReceiptInvalid(reason) => {
                write!(formatter, "promotion receipt is invalid: {reason}")
            }
            Self::RecoveryRequired(reason) => {
                write!(formatter, "promotion recovery_required: {reason}")
            }
            Self::Preview(error) => error.fmt(formatter),
            Self::PreviewDigestMismatch { expected, actual } => write!(
                formatter,
                "promotion preview changed before mutation: expected {expected}, actual {actual}"
            ),
            Self::NotEligible(reason) => {
                write!(
                    formatter,
                    "promotion is not eligible for local apply: {reason}"
                )
            }
            Self::MissingDerivedPrincipal => formatter
                .write_str("linked live claim must carry a principal matching the isolation agent"),
            Self::UnsupportedEffect(reason) => {
                write!(formatter, "unsupported promotion effect: {reason}")
            }
            Self::Payload(reason) => write!(formatter, "promotion payload failed: {reason}"),
            Self::Effect(reason) => write!(formatter, "promotion effect failed: {reason}"),
            Self::Readback(reason) => write!(formatter, "promotion readback failed: {reason}"),
        }
    }
}

impl std::error::Error for PromotionApplyError {}

impl From<PromotionPreviewError> for PromotionApplyError {
    fn from(value: PromotionPreviewError) -> Self {
        Self::Preview(value)
    }
}

impl std::fmt::Display for PromotionPreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoloProfileRequired => formatter
                .write_str("promotion preview requires the solo_cooperative readiness profile"),
            Self::ActiveObjectiveRequired => {
                formatter.write_str("promotion preview requires an active accepted objective")
            }
            Self::IsolationDirectory { path, source } => write!(
                formatter,
                "isolation directory {} is unavailable: {source}",
                path.display()
            ),
            Self::IsolationDocument { path, source } => write!(
                formatter,
                "isolation document {} is invalid: {source}",
                path.display()
            ),
            Self::TooManyIsolationDocuments => {
                formatter.write_str("isolation registry exceeds the bounded preview document limit")
            }
            Self::IsolationNotFound(id) => write!(formatter, "isolation {id} was not found"),
            Self::DuplicateIsolationId(id) => {
                write!(formatter, "isolation id {id} is not unique")
            }
            Self::IsolationInvalid(reason) => {
                write!(formatter, "isolation contract is invalid: {reason}")
            }
            Self::IsolationNotActive(id) => {
                write!(formatter, "isolation {id} is not active")
            }
            Self::SourceRoot { path, source } => {
                write!(
                    formatter,
                    "source root {} is invalid: {source}",
                    path.display()
                )
            }
            Self::SourceAliasesDestination => {
                formatter.write_str("isolation source overlaps the canonical destination tree")
            }
            Self::NonUtf8Path(path) => {
                write!(formatter, "promotion path {} is not UTF-8", path.display())
            }
            Self::Snapshot(error) => write!(formatter, "retained snapshot failed: {error}"),
            Self::SnapshotBindingMismatch => formatter
                .write_str("governance snapshot differs from the retained destination snapshot"),
            Self::ClaimProjection(source) => {
                write!(formatter, "claim WAL projection failed: {source}")
            }
            Self::ClaimProjectionChanged => formatter
                .write_str("claim WAL projection changed while the preview was being derived"),
            Self::Planning(error) => error.fmt(formatter),
            Self::IsolationChanged => formatter
                .write_str("isolation registry changed while the preview was being derived"),
            Self::ClockOverflow => formatter.write_str("promotion preview clock exceeds i64"),
            Self::FreshnessExpiredDuringDerivation => formatter.write_str(
                "promotion preview freshness expired while the preview was being derived",
            ),
            Self::GitWorktree(reason) => {
                write!(
                    formatter,
                    "declared isolation Git worktree is invalid: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for PromotionPreviewError {}

impl From<RetainedProjectTreeError> for PromotionPreviewError {
    fn from(value: RetainedProjectTreeError) -> Self {
        Self::Snapshot(value)
    }
}

impl From<PromotionPlanningError> for PromotionPreviewError {
    fn from(value: PromotionPlanningError) -> Self {
        Self::Planning(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IsolationSelection {
    relative_path: String,
    raw_digest: String,
    contract: IsolationContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ReplacementWorkspaceGapCode {
    IsolationRegistryInvalid,
    IsolationConflict,
    WorktreeMissing,
    GitWorktreeMismatch,
    PromotionStateInvalid,
    PromotionRequiresSoloProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReplacementWorkspaceGap {
    pub code: ReplacementWorkspaceGapCode,
    pub blocking: bool,
    pub summary: String,
    pub isolation_id: Option<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReplacementIsolationValidation {
    Valid,
    ProposedNotCreated,
    RetiredWorktreeAbsent,
    Missing,
    Mismatched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReplacementIsolationInspection {
    pub contract_path: String,
    pub contract_digest: String,
    pub contract: IsolationContract,
    pub declared_worktree: String,
    pub validation: ReplacementIsolationValidation,
    pub git: Option<PromotionGitWorktreeBinding>,
    pub gap_codes: Vec<ReplacementWorkspaceGapCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReplacementPromotionStatus {
    NotStarted,
    Recoverable,
    Completed,
    BlockedCorrupt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReplacementPromotionInspection {
    pub isolation_id: StableId,
    pub status: ReplacementPromotionStatus,
    pub preview_digest: Option<String>,
    pub receipt_digest: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct ReplacementWorkspaceInspection {
    pub isolations: Vec<ReplacementIsolationInspection>,
    pub promotions: Vec<ReplacementPromotionInspection>,
    pub gaps: Vec<ReplacementWorkspaceGap>,
}

/// Kernel-only prepared promotion observation. It deliberately implements
/// neither `Clone` nor Serde and retains the exact source-tree bytes that may
/// later become effect payloads.
pub(super) struct PreparedPromotion {
    pub(super) preview: GovernedPromotionPreview,
    pub(super) source_tree: RetainedProjectTree,
    pub(super) derived_principal_id: Option<forge_core_contracts::PrincipalId>,
}

pub(super) fn preview_governed_promotion(
    binding: &WorkflowGovernanceProjectBinding,
    isolation_id: &StableId,
    guidance: &WorkflowGovernanceGuidance,
    destination_tree: &RetainedProjectTree,
    now: u64,
) -> Result<GovernedPromotionPreview, PromotionPreviewError> {
    derive_governed_promotion(binding, isolation_id, guidance, destination_tree, now, None)
        .map(|prepared| prepared.preview)
}

pub(super) fn prepare_governed_promotion_with_claim_projection(
    binding: &WorkflowGovernanceProjectBinding,
    isolation_id: &StableId,
    guidance: &WorkflowGovernanceGuidance,
    destination_tree: &RetainedProjectTree,
    now: u64,
    claim_projection: &ClaimWalProjection,
) -> Result<PreparedPromotion, PromotionPreviewError> {
    derive_governed_promotion(
        binding,
        isolation_id,
        guidance,
        destination_tree,
        now,
        Some(claim_projection),
    )
}

fn derive_governed_promotion(
    binding: &WorkflowGovernanceProjectBinding,
    isolation_id: &StableId,
    guidance: &WorkflowGovernanceGuidance,
    destination_tree: &RetainedProjectTree,
    now: u64,
    retained_claim_projection: Option<&ClaimWalProjection>,
) -> Result<PreparedPromotion, PromotionPreviewError> {
    if guidance.readiness_profile != WorkflowReadinessProfile::SoloCooperative {
        return Err(PromotionPreviewError::SoloProfileRequired);
    }
    let objective = guidance
        .active_cooperative_objective
        .as_ref()
        .ok_or(PromotionPreviewError::ActiveObjectiveRequired)?;
    if guidance.snapshot_digest != destination_tree.regular_file_snapshot_digest() {
        return Err(PromotionPreviewError::SnapshotBindingMismatch);
    }

    let isolation = load_active_isolation(&binding.state_root, isolation_id)?;
    let destination_root = canonical_directory(&binding.project_root, "destination root")?;
    let declared_source =
        declared_worktree_candidate(&binding.project_root, &isolation.contract.worktree_path)?;
    let source_root = canonical_directory(&declared_source, "isolation worktree")?;
    if source_root == destination_root
        || source_root.starts_with(&destination_root)
        || destination_root.starts_with(&source_root)
    {
        return Err(PromotionPreviewError::SourceAliasesDestination);
    }
    let git_observation = observe_git_worktree(
        &source_root,
        &destination_root,
        &isolation.contract.branch_name,
    )?;
    let source_tree = RetainedProjectTree::capture(
        &source_root,
        MAX_PROMOTION_SNAPSHOT_ENTRIES,
        MAX_PROMOTION_SNAPSHOT_BYTES,
    )?;
    if source_tree.aliases_root(destination_tree)?
        || source_tree.shares_regular_file_object_with(destination_tree)?
    {
        return Err(PromotionPreviewError::SourceAliasesDestination);
    }

    let source_files = inventory_files(&source_tree);
    let destination_files = inventory_files(destination_tree);
    let source_directories = inventory_directories(&source_tree);
    let destination_directories = inventory_directories(destination_tree);
    let source_excluded_roots = excluded_root_bindings(&source_root)?;
    let destination_excluded_roots = excluded_root_bindings(&destination_root)?;
    let mut diff = derive_promotion_diff(
        &source_files,
        &destination_files,
        &source_directories,
        &destination_directories,
    )?;
    for excluded in &source_excluded_roots {
        if excluded.present && excluded.kind == PromotionExcludedRootKind::BuildOrDependencyCache {
            diff.unsupported_effects.push(PromotionUnsupportedEffect {
                path: RepoPath(excluded.name.clone()),
                kind: PromotionUnsupportedEffectKind::ExcludedSourceRootContent,
                detail: format!(
                    "source root {} is deliberately outside the promotable snapshot; remove or relocate it before apply authority can be prepared",
                    excluded.name
                ),
            });
        }
    }
    diff.unsupported_effects.sort_by(|left, right| {
        left.path
            .0
            .cmp(&right.path.0)
            .then_with(|| left.kind.cmp(&right.kind))
    });

    source_tree.revalidate()?;
    destination_tree.revalidate()?;
    if observe_git_worktree(
        &source_root,
        &destination_root,
        &isolation.contract.branch_name,
    )? != git_observation
    {
        return Err(PromotionPreviewError::GitWorktree(
            "Git worktree metadata changed during retained capture".to_owned(),
        ));
    }

    let final_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| PromotionPreviewError::ClockOverflow)?
        .as_secs()
        .max(now);

    let claim_wal_path = binding.state_root.join(CLAIM_WAL_RELATIVE_PATH);
    let claim_lock_path = binding.state_root.join(CLAIM_WAL_LOCK_RELATIVE_PATH);
    let claim_wal_present = path_exists_no_follow(&claim_wal_path)?;
    let claim_lock_present = path_exists_no_follow(&claim_lock_path)?;
    if claim_wal_present && !claim_lock_present {
        return Err(PromotionPreviewError::ClaimProjection(
            "active claim WAL exists without its pre-existing lock; read-only preview refuses to create authority state"
                .to_owned(),
        ));
    }
    let claim_projection = if let Some(retained) = retained_claim_projection {
        Some(retained.clone())
    } else {
        claim_lock_present
            .then(|| {
                project_existing_claim_wal(
                    &binding.state_root,
                    &ClaimWalProjectionOptions {
                        repair: false,
                        stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
                    },
                )
                .map_err(|error| PromotionPreviewError::ClaimProjection(error.to_string()))
            })
            .transpose()?
    };
    let claim_projection_digest = match &claim_projection {
        Some(projection) => promotion_domain_digest("promotion.claim_projection.v1", projection)?,
        None => promotion_domain_digest(
            "promotion.claim_projection_absent.v1",
            &("claim_wal_absent_v1", 0_u64),
        )?,
    };
    let now_i64 = i64::try_from(final_now).map_err(|_| PromotionPreviewError::ClockOverflow)?;
    let active_claim_ids = claim_projection
        .as_ref()
        .map(|projection| {
            projection
                .active_by_claim_id
                .values()
                .filter(|claim| is_live(&claim.claim_contract, now_i64))
                .map(|claim| claim.claim_contract.id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let linked_claim_id = isolation
        .contract
        .claim_id
        .as_ref()
        .map(|id| ClaimId(id.0.clone()));
    let linked_claim_contract = linked_claim_id.as_ref().and_then(|linked| {
        claim_projection.as_ref().and_then(|projection| {
            projection
                .active_by_claim_id
                .get(&linked.0)
                .map(|claim| claim.claim_contract.clone())
                .filter(|claim| {
                    claim.claim.claimant_agent_id == isolation.contract.agent_id
                        && is_live(claim, now_i64)
                })
        })
    });
    let linked_claim_current = linked_claim_contract.is_some();
    let linked_claim_principal_id = linked_claim_contract
        .as_ref()
        .and_then(|claim| claim.claim.claimant_principal_id.clone());
    let linked_claim_valid_through = linked_claim_contract.as_ref().and_then(|claim| {
        rfc3339_to_unix(&claim.lease.expires_at).and_then(|value| u64::try_from(value).ok())
    });
    let current_claims = claim_projection
        .as_ref()
        .map(|projection| {
            projection
                .latest_by_claim_id
                .values()
                .map(|claim| claim.claim_contract.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut governed_paths = Vec::new();
    let mut ungoverned_paths = Vec::new();
    let mut path_attribution = Vec::new();
    let mut conflicts = Vec::new();
    for path in &diff.write_set {
        if let WriteCheck::Blocked { blocks } = check_write_against_claims(
            std::slice::from_ref(path),
            &isolation.contract.agent_id,
            &current_claims,
            now_i64,
        ) {
            conflicts.extend(blocks.into_iter().map(|blocked| PromotionClaimConflict {
                path: blocked.blocked_path,
                blocking_claim_id: blocked.blocking_claim_id,
                blocking_agent_id: blocked.claimant,
            }));
        }
        let linked_slice: &[forge_core_contracts::ClaimContract] = linked_claim_contract
            .as_ref()
            .map_or(&[], std::slice::from_ref);
        let governing_linked_claim_id = match check_write_against_claims(
            std::slice::from_ref(path),
            &isolation.contract.agent_id,
            linked_slice,
            now_i64,
        ) {
            WriteCheck::Ok {
                governed_by_self, ..
            } if !governed_by_self.is_empty() => {
                governed_paths.push(path.clone());
                linked_claim_id.clone()
            }
            _ => {
                ungoverned_paths.push(path.clone());
                None
            }
        };
        path_attribution.push(PromotionPathClaimAttribution {
            path: path.clone(),
            governing_linked_claim_id,
        });
    }
    governed_paths.sort_by(|left, right| left.0.cmp(&right.0));
    governed_paths.dedup();
    ungoverned_paths.sort_by(|left, right| left.0.cmp(&right.0));
    ungoverned_paths.dedup();
    path_attribution.sort_by(|left, right| left.path.0.cmp(&right.path.0));
    conflicts.sort_by(|left, right| {
        left.path
            .0
            .cmp(&right.path.0)
            .then_with(|| left.blocking_claim_id.0.cmp(&right.blocking_claim_id.0))
    });
    let write_claim_coverage = PromotionWriteClaimCoverage {
        status: if !conflicts.is_empty() {
            PromotionWriteClaimCoverageStatus::ConflictingClaim
        } else if ungoverned_paths.is_empty() {
            PromotionWriteClaimCoverageStatus::FullyGovernedByIsolationOwner
        } else {
            PromotionWriteClaimCoverageStatus::PartiallyOrFullyUngoverned
        },
        governed_paths,
        ungoverned_paths,
        path_attribution,
    };

    let cooperative_max_age_seconds = guidance
        .cooperative_evidence_action_packet
        .as_ref()
        .map(|packet| packet.route.max_age_seconds);
    let mut latest_supporting_evidence_valid_through: Option<u64> = None;
    let mut evidence_records = guidance
        .cooperative_evidence
        .iter()
        .filter(|evidence| {
            evidence.historical_disposition == WorkflowCooperativeEvidenceDisposition::Admitted
        })
        .map(|evidence| {
            let evidence_valid_through = evidence
                .admitted_evidence
                .as_ref()
                .zip(cooperative_max_age_seconds)
                .and_then(|(admitted, max_age)| {
                    admitted.readback_observed_at_unix.checked_add(max_age)
                });
            let current_status = if evidence.current_status
                == WorkflowCooperativeEvidenceCurrentStatus::Supporting
                && evidence_valid_through.is_some_and(|valid_through| final_now <= valid_through)
            {
                latest_supporting_evidence_valid_through = match (
                    latest_supporting_evidence_valid_through,
                    evidence_valid_through,
                ) {
                    (Some(current), Some(candidate)) => Some(current.max(candidate)),
                    (None, candidate) => candidate,
                    (current, None) => current,
                };
                WorkflowCooperativeEvidenceCurrentStatus::Supporting
            } else if evidence.current_status == WorkflowCooperativeEvidenceCurrentStatus::Rejected
            {
                WorkflowCooperativeEvidenceCurrentStatus::Rejected
            } else {
                WorkflowCooperativeEvidenceCurrentStatus::Stale
            };
            PromotionEvidenceRecordBinding {
                record_digest: evidence.record_digest.clone(),
                offer_digest: evidence.offer_digest.clone(),
                historical_disposition: evidence.historical_disposition,
                current_status,
                supports_cooperative_claim_ref: (current_status
                    == WorkflowCooperativeEvidenceCurrentStatus::Supporting)
                    .then(|| evidence.supports_cooperative_claim_ref.clone())
                    .flatten(),
                does_not_satisfy_source_claim_ref: evidence
                    .does_not_satisfy_source_claim_ref
                    .clone(),
                proves: if current_status == WorkflowCooperativeEvidenceCurrentStatus::Supporting {
                    evidence.proves.clone()
                } else {
                    Vec::new()
                },
                does_not_prove: evidence.does_not_prove.clone(),
            }
        })
        .collect::<Vec<_>>();
    evidence_records.sort_by(|left, right| left.record_digest.cmp(&right.record_digest));
    let supporting_record_count = evidence_records
        .iter()
        .filter(|record| {
            record.current_status == WorkflowCooperativeEvidenceCurrentStatus::Supporting
        })
        .count();
    let evidence_set_digest =
        promotion_domain_digest("promotion.cooperative_evidence_set.v1", &evidence_records)?;
    let evidence = PromotionEvidenceSetBinding {
        admitted_records: evidence_records,
        evidence_set_digest,
        supporting_record_count,
    };
    let valid_through_unix = match (
        linked_claim_valid_through,
        latest_supporting_evidence_valid_through,
    ) {
        (Some(claim), Some(evidence)) => Some(claim.min(evidence)),
        (claim, None) => claim,
        (None, evidence) => evidence,
    };

    let mut assurance_claim_coverage = guidance
        .simulation
        .candidate_claim_results
        .iter()
        .map(|claim| {
            let mut accepted = claim.accepted_evidence_refs.clone();
            accepted.sort();
            let mut rejected = claim.rejected_evidence_refs.clone();
            rejected.sort();
            PromotionAssuranceClaimCoverage {
                claim_ref: StableId(claim.claim_id.clone()),
                status: assurance_claim_status(claim.status),
                accepted_evidence_refs: accepted,
                rejected_evidence_refs: rejected,
            }
        })
        .collect::<Vec<_>>();
    assurance_claim_coverage.sort_by(|left, right| left.claim_ref.cmp(&right.claim_ref));
    let blocking_source_claim_refs = assurance_claim_coverage
        .iter()
        .filter(|claim| {
            matches!(
                claim.status,
                PromotionAssuranceClaimStatus::Disproven
                    | PromotionAssuranceClaimStatus::Contradictory
            )
        })
        .map(|claim| claim.claim_ref.0.clone())
        .collect::<Vec<_>>();
    let carried_assurance_gaps = assurance_claim_coverage
        .iter()
        .filter(|claim| {
            matches!(
                claim.status,
                PromotionAssuranceClaimStatus::Unknown | PromotionAssuranceClaimStatus::Supported
            )
        })
        .map(|claim| PromotionCarriedAssuranceGap {
            claim_ref: claim.claim_ref.clone(),
            status: claim.status,
            accepted_evidence_refs: claim.accepted_evidence_refs.clone(),
            rejected_evidence_refs: claim.rejected_evidence_refs.clone(),
            cooperative_evidence_is_independent_verification: false,
        })
        .collect::<Vec<_>>();

    let conflicting_paths = conflicts
        .iter()
        .map(|conflict| conflict.path.clone())
        .collect::<Vec<_>>();
    let readiness = evaluate_promotion_readiness(&PromotionReadinessInput {
        diff: &diff.diff,
        has_linked_claim: linked_claim_current,
        ungoverned_paths: &write_claim_coverage.ungoverned_paths,
        conflicting_paths: &conflicting_paths,
        unsupported_effects: &diff.unsupported_effects,
        supporting_cooperative_evidence: evidence.supporting_record_count,
        blocking_source_claim_refs: &blocking_source_claim_refs,
        has_linked_claim_principal: linked_claim_principal_id.is_some(),
        open_objective_uncertainties: objective.proposal.open_uncertainties.len(),
    });

    let source_snapshot = snapshot_binding(&source_root, &source_tree, source_excluded_roots)?;
    let destination_snapshot = snapshot_binding(
        &destination_root,
        destination_tree,
        destination_excluded_roots,
    )?;
    let source = PromotionSourceBinding {
        isolation_id: isolation.contract.id.clone(),
        isolation_contract_digest: isolation.raw_digest.clone(),
        isolation_contract_relative_path: isolation.relative_path.clone(),
        isolation_status: IsolationStatus::Active,
        agent_id: isolation.contract.agent_id.clone(),
        linked_claim_id: linked_claim_id.clone(),
        linked_claim_principal_id: linked_claim_principal_id.clone(),
        declared_worktree_path: isolation.contract.worktree_path.clone(),
        git_worktree: git_observation.binding.clone(),
        snapshot: source_snapshot,
    };
    let destination = PromotionDestinationBinding {
        project_id: binding.project_id.clone(),
        snapshot: destination_snapshot,
    };
    let objective_binding = PromotionObjectiveBinding {
        objective_id: objective.objective_id.clone(),
        revision: objective.revision,
        objective_digest: objective.objective_digest.clone(),
        assurance_epoch: objective.assurance_epoch,
        accepted_record_digest: objective.accepted_record_digest.clone(),
        accepted_record_sequence: objective.accepted_sequence,
        accepted_state_version: objective.accepted_state_version,
    };
    let governance = PromotionGovernanceBinding {
        readiness_profile: guidance.readiness_profile,
        effective_bundle_digest: guidance
            .effective
            .effective_runtime_bundle
            .bundle_digest
            .clone(),
        selected_policy_ref: guidance.selected_policy_ref.clone(),
        ledger_head_digest: guidance.ledger_head_digest.clone(),
        state_version: guidance.state_version,
    };
    let claims = PromotionClaimSetBinding {
        claim_projection_digest,
        last_applied_sequence: claim_projection
            .as_ref()
            .map_or(0, |projection| projection.last_applied_seq),
        linked_claim_id,
        active_claim_ids,
    };
    let objective_coverage = PromotionObjectiveCoverage {
        status:
            PromotionObjectiveCoverageStatus::BoundToAcceptedObjectiveSemanticCoverageNotInferred,
        semantic_coverage_caller_assertion_accepted: false,
        open_uncertainty_count: objective.proposal.open_uncertainties.len(),
    };
    let mut preview = GovernedPromotionPreview {
        schema_version: GOVERNED_PROMOTION_PREVIEW_SCHEMA_VERSION.to_owned(),
        preview_id: StableId(String::new()),
        preview_digest: String::new(),
        observed_at_unix: final_now,
        valid_through_unix,
        authority: GovernedPromotionPreviewAuthority::ReadOnlyCandidateNoApplyAuthority,
        status: readiness.status,
        apply_eligibility: readiness.apply_eligibility,
        canonical_mutation_performed: false,
        forge_state_mutation_performed: false,
        source,
        destination,
        objective: objective_binding,
        governance,
        evidence,
        claims,
        diff: diff.diff,
        write_set: diff.write_set,
        diff_digest: diff.diff_digest,
        write_set_digest: diff.write_set_digest,
        predicted_result_regular_file_set_digest: diff.predicted_result_regular_file_set_digest,
        objective_coverage,
        assurance_claim_coverage,
        carried_assurance_gaps,
        write_claim_coverage,
        conflicts,
        unsupported_effects: diff.unsupported_effects,
        unresolved_gaps: readiness.unresolved_gaps,
    };
    let mut stable_preview = preview.clone();
    stable_preview.observed_at_unix = 0;
    let preview_digest = promotion_domain_digest("promotion.preview.v1", &stable_preview)?;
    preview.preview_id = StableId(format!(
        "promotion.preview.{}",
        preview_digest.trim_start_matches("sha256:")
    ));
    preview.preview_digest = preview_digest;

    source_tree.revalidate()?;
    destination_tree.revalidate()?;
    if observe_git_worktree(
        &source_root,
        &destination_root,
        &isolation.contract.branch_name,
    )? != git_observation
    {
        return Err(PromotionPreviewError::GitWorktree(
            "Git worktree metadata changed during preview".to_owned(),
        ));
    }
    let current_isolation = load_active_isolation(&binding.state_root, isolation_id)?;
    if current_isolation != isolation {
        return Err(PromotionPreviewError::IsolationChanged);
    }
    if retained_claim_projection.is_some() {
        // The caller retains and revalidates the exact claim lock; reacquiring
        // here would self-deadlock and would weaken the linear authority chain.
    } else if claim_projection.is_none() {
        if path_exists_no_follow(&claim_wal_path)? || path_exists_no_follow(&claim_lock_path)? {
            return Err(PromotionPreviewError::ClaimProjection(
                "claim authority appeared while the read-only preview was being derived".to_owned(),
            ));
        }
    } else {
        let current_claim_projection = project_existing_claim_wal(
            &binding.state_root,
            &ClaimWalProjectionOptions {
                repair: false,
                stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
            },
        )
        .map_err(|error| PromotionPreviewError::ClaimProjection(error.to_string()))?;
        if promotion_domain_digest("promotion.claim_projection.v1", &current_claim_projection)?
            != preview.claims.claim_projection_digest
        {
            return Err(PromotionPreviewError::ClaimProjectionChanged);
        }
    }
    let return_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| PromotionPreviewError::ClockOverflow)?
        .as_secs();
    if preview.status == forge_core_contracts::GovernedPromotionPreviewStatus::Reviewable
        && preview
            .valid_through_unix
            .is_some_and(|valid_through| return_now > valid_through)
    {
        return Err(PromotionPreviewError::FreshnessExpiredDuringDerivation);
    }
    Ok(PreparedPromotion {
        preview,
        source_tree,
        derived_principal_id: linked_claim_principal_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PromotionReplayIntent {
    schema_version: String,
    expected_preview_digest: String,
    principal_id: PrincipalId,
    transaction_id: StableId,
    effect_id: StableId,
    replay: PromotionReplayBinding,
    provenance_digest: String,
    publication_capability_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview: Option<GovernedPromotionPreview>,
}

#[derive(Debug, Default)]
struct PromotionStateBudget {
    documents: usize,
    bytes: u64,
}

pub(super) fn inspect_promotion_retry_under_lock(
    binding: &WorkflowGovernanceProjectBinding,
    effect_lock: &EffectStoreLock,
    isolation_id: &StableId,
    expected_preview_digest: &str,
    allow_incomplete_intent: bool,
) -> Result<Option<GovernedPromotionApplication>, PromotionApplyError> {
    let receipt_name = promotion_state_leaf_name(expected_preview_digest)?;
    let io = effect_lock
        .retained_store_io()
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    let receipts = io
        .retain_subdirectory(Path::new("receipts"))
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    if let Some(mut witness) = receipts
        .read_optional_bounded(Path::new(&receipt_name), PROMOTION_RECEIPT_MAX_BYTES)
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?
    {
        let receipt: GovernedPromotionReceipt = serde_json::from_slice(witness.raw_bytes())
            .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
        verify_promotion_receipt(&receipt, expected_preview_digest)?;
        if receipt.preview.source.isolation_id != *isolation_id {
            return Err(PromotionApplyError::ReceiptInvalid(
                "receipt isolation differs from the requested isolation".to_owned(),
            ));
        }
        verify_consumed_replay_binding_under_lock(binding, effect_lock, &receipt)?;
        witness
            .revalidate()
            .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
        verify_committed_receipt_readback(binding, &receipt)?;
        return Ok(Some(GovernedPromotionApplication {
            status: GovernedPromotionApplyStatus::AlreadyCommitted,
            canonical_mutation_performed: false,
            receipt,
        }));
    }
    let intents = io
        .retain_subdirectory(Path::new("intents"))
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    if intents
        .read_optional_bounded(Path::new(&receipt_name), PROMOTION_RECEIPT_MAX_BYTES)
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?
        .is_some()
    {
        if allow_incomplete_intent {
            return Ok(None);
        }
        return Err(PromotionApplyError::RecoveryRequired(format!(
            "durable intent exists without receipt for {expected_preview_digest}; do not re-execute"
        )));
    }
    Ok(None)
}

pub(super) fn validate_expected_preview_digest(
    expected_preview_digest: &str,
) -> Result<(), PromotionApplyError> {
    promotion_state_leaf_name(expected_preview_digest).map(|_| ())
}

pub(super) fn recovery_preview_under_lock(
    binding: &WorkflowGovernanceProjectBinding,
    effect_lock: &EffectStoreLock,
    isolation_id: &StableId,
    expected_preview_digest: &str,
) -> Result<Option<GovernedPromotionPreview>, PromotionApplyError> {
    let intent = load_promotion_intent_under_lock(effect_lock, expected_preview_digest)?;
    validate_intent_request(&intent, isolation_id, expected_preview_digest)?;
    if let Some(preview) = intent.preview {
        return Ok(Some(preview));
    }
    let inspection = inspect_split_root_effect_transaction_under_lock(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        PROMOTION_EFFECT_WAL_RELATIVE_PATH,
        &intent.transaction_id.0,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    inspection
        .map(|inspection| preview_from_effect_inspection(&inspection))
        .transpose()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn recover_promotion_under_lock(
    binding: &WorkflowGovernanceProjectBinding,
    isolation_id: &StableId,
    expected_preview_digest: &str,
    fallback_prepared: Option<PreparedPromotion>,
    destination_tree: &mut RetainedProjectTree,
    effect_lock: &EffectStoreLock,
) -> Result<GovernedPromotionApplication, PromotionApplyError> {
    let intent = load_promotion_intent_under_lock(effect_lock, expected_preview_digest)?;
    validate_intent_request(&intent, isolation_id, expected_preview_digest)?;
    let inspection = inspect_split_root_effect_transaction_under_lock(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        PROMOTION_EFFECT_WAL_RELATIVE_PATH,
        &intent.transaction_id.0,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let stored_preview = intent.preview.clone().or_else(|| {
        inspection
            .as_ref()
            .and_then(|inspection| preview_from_effect_inspection(inspection).ok())
    });
    let prepared = if let Some(preview) = stored_preview {
        prepare_recovery_from_stored_preview(
            binding,
            isolation_id,
            expected_preview_digest,
            preview,
            destination_tree,
        )?
    } else {
        let prepared = fallback_prepared.ok_or_else(|| {
            PromotionApplyError::RecoveryRequired(
                "legacy v1 intent has no effect begin and current source/destination could not reconstruct the exact approved preview; preserve both roots and retry after restoring the expected state"
                    .to_owned(),
            )
        })?;
        if prepared.preview.preview_digest != expected_preview_digest {
            return Err(PromotionApplyError::RecoveryRequired(
                "legacy v1 intent reconstructed a different preview; no recovery write was attempted"
                    .to_owned(),
            ));
        }
        prepared
    };
    validate_recovery_preview_identity(&prepared.preview, expected_preview_digest)?;
    if inspection.is_none() {
        validate_pre_begin_destination_exact(binding, destination_tree, &prepared.preview)?;
    }
    validate_recovery_destination(destination_tree, &prepared.source_tree, &prepared.preview)?;
    let (effect, payloads, applied_files) = promotion_effect_and_payloads(&prepared)?;
    let legacy_v1_pre_begin =
        intent.schema_version == "governed_promotion_intent_v1" && inspection.is_none();
    let original_provenance = if legacy_v1_pre_begin {
        // v1 intentionally did not retain the preview. Its self-digested
        // intent remains authoritative, but a fresh preview with the same
        // semantic digest cannot recreate the historical observed_at value.
        // Do not pretend that historical provenance was reconstructed.
        None
    } else {
        let reconstructed = promotion_provenance_from_intent(&intent, &prepared.preview, &effect)?;
        if reconstructed.digest != intent.provenance_digest {
            return Err(PromotionApplyError::RecoveryRequired(
                "durable intent provenance differs from its exact historical preview/effect binding"
                    .to_owned(),
            ));
        }
        Some(reconstructed)
    };
    let (provenance, publication_capability_digest, recovery_execution) = if let Some(inspection) =
        &inspection
    {
        let original_provenance = original_provenance.as_ref().ok_or_else(|| {
            PromotionApplyError::RecoveryRequired(
                "legacy intent has an effect begin but no reconstructable historical preview"
                    .to_owned(),
            )
        })?;
        if inspection.provenance == *original_provenance {
            (
                original_provenance.clone(),
                intent.publication_capability_digest.clone(),
                None,
            )
        } else {
            let publication_capability_digest =
                recovery_publication_capability_from_wal(&inspection.provenance)?;
            let recovery_execution = recovery_execution_binding(&intent);
            let expected_recovery = pre_begin_recovery_provenance(
                &intent,
                &prepared.preview,
                &effect,
                &publication_capability_digest,
                &recovery_execution,
            )?;
            if inspection.provenance != expected_recovery {
                return Err(PromotionApplyError::RecoveryRequired(
                        "effect WAL recovery provenance does not exactly link to the durable original intent"
                            .to_owned(),
                    ));
            }
            (
                inspection.provenance.clone(),
                publication_capability_digest,
                Some(recovery_execution),
            )
        }
    } else {
        let publication_capability_digest = destination_tree
            .exact_mutation_capability_digest()
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
        let recovery_execution = recovery_execution_binding(&intent);
        let provenance = pre_begin_recovery_provenance(
            &intent,
            &prepared.preview,
            &effect,
            &publication_capability_digest,
            &recovery_execution,
        )?;
        (
            provenance,
            publication_capability_digest,
            Some(recovery_execution),
        )
    };
    let replay_binding = EffectReplayCommitBinding::new(
        intent.replay.key_hash.clone(),
        intent.replay.intent_digest.clone(),
        intent.replay.commit_digest.clone(),
        intent.replay.reservation_revision,
    );
    let (canonical_mutation_performed, stage) = if let Some(inspection) = &inspection {
        if inspection.provenance != provenance
            || inspection.replay_binding != replay_binding
            || inspection.effect_id != effect.tool_effect_contract.id
        {
            return Err(PromotionApplyError::RecoveryRequired(
                "effect WAL begin contradicts the durable promotion intent".to_owned(),
            ));
        }
        if inspection.stage == SplitRootEffectTransactionStage::Begun {
            // Recovery can write only while the exact still-reserved replay
            // authority remains locked. Missing, consumed, or mismatched replay
            // state therefore blocks before another canonical or effect-WAL
            // write.
            let replay_guard = acquire_replay_commit_guard(
                &binding.state_root,
                effect_lock,
                PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
                &intent.principal_id,
                PROMOTION_REPLAY_AUDIENCE,
                expected_preview_digest,
                &intent.replay.intent_digest,
                &intent.replay.commit_digest,
                intent.replay.reservation_revision,
            )
            .map_err(|error| {
                PromotionApplyError::RecoveryRequired(format!(
                    "incomplete promotion replay authority could not be retained before recovery: {error}"
                ))
            })?;
            let recovered = recover_existing_file_effect_transaction_to_retained_project_tree(
                &binding.state_root,
                destination_tree,
                replay_guard.effect_lock(),
                PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
                &effect,
                &payloads,
                PROMOTION_EFFECT_WAL_RELATIVE_PATH,
                &intent.transaction_id.0,
                &provenance,
                &replay_binding,
            )
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
            if recovered.status != SplitRootEffectTransactionStage::Committed {
                return Err(PromotionApplyError::RecoveryRequired(format!(
                    "incomplete promotion did not converge to a commit: {:?}",
                    recovered.status
                )));
            }
            let replay_result = replay_guard.consume().map_err(|error| {
                PromotionApplyError::RecoveryRequired(format!(
                    "recovered effect committed but replay consume failed: {error}"
                ))
            })?;
            debug_test_promotion_crash("after_replay_consume");
            append_effect_replay_completion_under_lock(
                &binding.state_root,
                effect_lock,
                PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
                PROMOTION_EFFECT_WAL_RELATIVE_PATH,
                &intent.transaction_id.0,
                &intent.effect_id,
                &replay_binding,
                &replay_result,
                true,
            )
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
            (
                recovered.canonical_mutation_performed,
                SplitRootEffectTransactionStage::ReplayConsumed,
            )
        } else {
            let recovered = recover_existing_file_effect_transaction_to_retained_project_tree(
                &binding.state_root,
                destination_tree,
                effect_lock,
                PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
                &effect,
                &payloads,
                PROMOTION_EFFECT_WAL_RELATIVE_PATH,
                &intent.transaction_id.0,
                &provenance,
                &replay_binding,
            )
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
            (recovered.canonical_mutation_performed, recovered.status)
        }
    } else {
        match reserve_replay_nonce_under_effect_lock(
            &binding.state_root,
            effect_lock,
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            &intent.principal_id,
            PROMOTION_REPLAY_AUDIENCE,
            expected_preview_digest,
            &intent.replay.intent_digest,
            &intent.replay.commit_digest,
        ) {
            Ok(reservation) => {
                if reservation.reservation.key_hash != intent.replay.key_hash
                    || reservation.reservation.revision != intent.replay.reservation_revision
                {
                    return Err(PromotionApplyError::RecoveryRequired(
                        "reconstructed replay reservation differs from the durable intent"
                            .to_owned(),
                    ));
                }
            }
            Err(ReplayWalError::DuplicateNonce { .. }) => {}
            Err(error) => {
                return Err(PromotionApplyError::RecoveryRequired(format!(
                    "legacy/pre-begin replay reservation could not be reconciled: {error}"
                )));
            }
        }
        let replay_guard = acquire_replay_commit_guard(
            &binding.state_root,
            effect_lock,
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            &intent.principal_id,
            PROMOTION_REPLAY_AUDIENCE,
            expected_preview_digest,
            &intent.replay.intent_digest,
            &intent.replay.commit_digest,
            intent.replay.reservation_revision,
        )
        .map_err(|error| {
            PromotionApplyError::RecoveryRequired(format!(
                "pre-begin replay authority could not be retained: {error}"
            ))
        })?;
        let effect_result = apply_existing_file_effect_transaction_to_retained_project_tree(
            &binding.state_root,
            destination_tree,
            replay_guard.effect_lock(),
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            &effect,
            &payloads,
            PROMOTION_EFFECT_WAL_RELATIVE_PATH,
            intent.transaction_id.0.clone(),
            provenance.clone(),
            replay_binding.clone(),
        );
        if effect_result.status != EffectApplicationStatus::Applied {
            return Err(PromotionApplyError::RecoveryRequired(format!(
                "pre-begin recovery effect did not commit: status={:?}, diagnostics={:?}",
                effect_result.status, effect_result.diagnostics
            )));
        }
        let replay_result = replay_guard.consume().map_err(|error| {
            PromotionApplyError::RecoveryRequired(format!(
                "recovered effect committed but replay consume failed: {error}"
            ))
        })?;
        debug_test_promotion_crash("after_replay_consume");
        append_effect_replay_completion_under_lock(
            &binding.state_root,
            effect_lock,
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            PROMOTION_EFFECT_WAL_RELATIVE_PATH,
            &intent.transaction_id.0,
            &intent.effect_id,
            &replay_binding,
            &replay_result,
            true,
        )
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
        (true, SplitRootEffectTransactionStage::ReplayConsumed)
    };
    if stage != SplitRootEffectTransactionStage::ReplayConsumed {
        let replay_result = consume_replay_key_hash_under_effect_lock(
            &binding.state_root,
            effect_lock,
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            &intent.replay.key_hash,
            &intent.replay.intent_digest,
            &intent.replay.commit_digest,
            intent.replay.reservation_revision,
        )
        .map_err(|error| {
            PromotionApplyError::RecoveryRequired(format!(
                "committed promotion replay consume failed: {error}"
            ))
        })?;
        debug_test_promotion_crash("after_replay_consume");
        append_effect_replay_completion_under_lock(
            &binding.state_root,
            effect_lock,
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            PROMOTION_EFFECT_WAL_RELATIVE_PATH,
            &intent.transaction_id.0,
            &intent.effect_id,
            &replay_binding,
            &replay_result,
            true,
        )
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    } else {
        verify_consumed_replay_key_hash_under_effect_lock(
            &binding.state_root,
            effect_lock,
            PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
            &intent.replay.key_hash,
            &intent.replay.intent_digest,
            &intent.replay.commit_digest,
            intent.replay.reservation_revision,
        )
        .map_err(|error| {
            PromotionApplyError::RecoveryRequired(format!(
                "effect replay completion contradicts replay WAL: {error}"
            ))
        })?;
    }

    destination_tree.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "recovered canonical readback failed: {error}"
        ))
    })?;
    verify_logical_result_matches_source(destination_tree, &prepared.source_tree)
        .map_err(PromotionApplyError::RecoveryRequired)?;
    let committed_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?
        .as_secs();
    let result_root = canonical_directory(&binding.project_root, "recovered result root")
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let result_snapshot = snapshot_binding(
        &result_root,
        destination_tree,
        excluded_root_bindings(&result_root)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let result_snapshot_digest =
        promotion_domain_digest("promotion.result_snapshot.v1", &result_snapshot)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    debug_test_promotion_crash("after_readback");
    let mut receipt = GovernedPromotionReceipt {
        schema_version: GOVERNED_PROMOTION_RECEIPT_SCHEMA_VERSION.to_owned(),
        receipt_digest: String::new(),
        committed_at_unix,
        preview: prepared.preview,
        derived_principal_id: intent.principal_id,
        transaction_id: intent.transaction_id,
        effect_id: intent.effect_id,
        provenance_digest: provenance.digest,
        publication_capability_digest,
        replay: intent.replay,
        recovery_execution,
        applied_files,
        result_snapshot,
        result_snapshot_digest,
        readback_verified: true,
    };
    receipt.receipt_digest = promotion_receipt_digest(&receipt)?;
    let state_leaf = promotion_state_leaf_name(expected_preview_digest)?;
    let receipt_bytes = serde_json_canonicalizer::to_vec(&receipt)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let io = effect_lock
        .retained_store_io()
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let receipts = io
        .retain_subdirectory(Path::new("receipts"))
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let mut receipt_witness = receipts
        .write_new_file_synced(
            Path::new(&state_leaf),
            &receipt_bytes,
            PROMOTION_RECEIPT_MAX_BYTES,
        )
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    receipt_witness
        .revalidate()
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    debug_test_promotion_crash("after_receipt");
    let persisted: GovernedPromotionReceipt =
        serde_json::from_slice(receipt_witness.raw_bytes())
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    verify_promotion_receipt(&persisted, expected_preview_digest)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    verify_consumed_replay_binding_under_lock(binding, effect_lock, &persisted)?;
    verify_committed_receipt_readback(binding, &persisted)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    Ok(GovernedPromotionApplication {
        status: GovernedPromotionApplyStatus::Recovered,
        canonical_mutation_performed,
        receipt: persisted,
    })
}

fn load_promotion_intent_under_lock(
    effect_lock: &EffectStoreLock,
    expected_preview_digest: &str,
) -> Result<PromotionReplayIntent, PromotionApplyError> {
    let state_leaf = promotion_state_leaf_name(expected_preview_digest)?;
    let io = effect_lock
        .retained_store_io()
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let intents = io
        .retain_subdirectory(Path::new("intents"))
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let mut witness = intents
        .read_optional_bounded(Path::new(&state_leaf), PROMOTION_RECEIPT_MAX_BYTES)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?
        .ok_or_else(|| {
            PromotionApplyError::RecoveryRequired(format!(
                "no durable promotion intent exists for {expected_preview_digest}"
            ))
        })?;
    let intent: PromotionReplayIntent =
        serde_json::from_slice(witness.raw_bytes()).map_err(|error| {
            PromotionApplyError::RecoveryRequired(format!(
                "durable promotion intent is corrupt or has an unsupported shape: {error}"
            ))
        })?;
    witness.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "durable promotion intent changed during recovery: {error}"
        ))
    })?;
    if !matches!(
        intent.schema_version.as_str(),
        "governed_promotion_intent_v1" | "governed_promotion_intent_v2"
    ) || (intent.schema_version == "governed_promotion_intent_v1" && intent.preview.is_some())
        || (intent.schema_version == "governed_promotion_intent_v2" && intent.preview.is_none())
    {
        return Err(PromotionApplyError::RecoveryRequired(format!(
            "unsupported or contradictory promotion intent schema {}",
            intent.schema_version
        )));
    }
    Ok(intent)
}

fn validate_intent_request(
    intent: &PromotionReplayIntent,
    isolation_id: &StableId,
    expected_preview_digest: &str,
) -> Result<(), PromotionApplyError> {
    let expected_hex = expected_preview_digest.trim_start_matches("sha256:");
    if intent.expected_preview_digest != expected_preview_digest
        || intent.transaction_id != StableId(format!("promotion.tx.{expected_hex}"))
        || intent.effect_id != StableId(format!("promotion.effect.{expected_hex}"))
        || intent.replay.audience != PROMOTION_REPLAY_AUDIENCE
        || intent.replay.reservation_revision != 1
        || !is_sha256_digest(&intent.replay.key_hash)
        || !is_sha256_digest(&intent.replay.intent_digest)
        || !is_sha256_digest(&intent.replay.commit_digest)
        || !is_sha256_digest(&intent.provenance_digest)
        || !is_sha256_digest(&intent.publication_capability_digest)
        || intent
            .preview
            .as_ref()
            .is_some_and(|preview| preview.source.isolation_id != *isolation_id)
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "durable intent bindings differ from the requested isolation/preview".to_owned(),
        ));
    }
    let expected_key = replay_nonce_key_hash(
        &intent.principal_id,
        PROMOTION_REPLAY_AUDIENCE,
        expected_preview_digest,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if expected_key != intent.replay.key_hash {
        return Err(PromotionApplyError::RecoveryRequired(
            "durable intent replay key differs from its principal/preview binding".to_owned(),
        ));
    }
    let mut canonical = intent.clone();
    canonical.replay.intent_digest.clear();
    let actual_intent = promotion_domain_digest("promotion.intent.v1", &canonical)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if actual_intent != intent.replay.intent_digest {
        return Err(PromotionApplyError::RecoveryRequired(
            "durable intent self-binding digest is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn preview_from_effect_inspection(
    inspection: &SplitRootEffectTransactionInspection,
) -> Result<GovernedPromotionPreview, PromotionApplyError> {
    let preview = inspection
        .provenance
        .document
        .get("preview")
        .ok_or_else(|| {
            PromotionApplyError::RecoveryRequired(
                "legacy v1 effect begin lacks its embedded approved preview".to_owned(),
            )
        })?;
    serde_json::from_value(preview.clone()).map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "legacy v1 effect begin contains an invalid approved preview: {error}"
        ))
    })
}

fn prepare_recovery_from_stored_preview(
    binding: &WorkflowGovernanceProjectBinding,
    isolation_id: &StableId,
    expected_preview_digest: &str,
    preview: GovernedPromotionPreview,
    destination_tree: &RetainedProjectTree,
) -> Result<PreparedPromotion, PromotionApplyError> {
    validate_recovery_preview_identity(&preview, expected_preview_digest)?;
    if preview.source.isolation_id != *isolation_id
        || preview.destination.project_id != binding.project_id
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "stored preview source/destination identity differs from the current project request"
                .to_owned(),
        ));
    }
    let isolation = load_active_isolation(&binding.state_root, isolation_id)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if isolation.raw_digest != preview.source.isolation_contract_digest
        || isolation.relative_path != preview.source.isolation_contract_relative_path
        || isolation.contract.agent_id != preview.source.agent_id
        || isolation.contract.worktree_path != preview.source.declared_worktree_path
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "active isolation contract differs from the durable approved preview".to_owned(),
        ));
    }
    let destination_root = canonical_directory(&binding.project_root, "recovery destination root")
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if destination_root.to_string_lossy().as_ref()
        != preview.destination.snapshot.canonical_root.as_str()
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "canonical destination root differs from the approved preview".to_owned(),
        ));
    }
    let declared_source =
        declared_worktree_candidate(&binding.project_root, &isolation.contract.worktree_path)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let source_root = canonical_directory(&declared_source, "recovery isolation worktree")
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let source_tree = RetainedProjectTree::capture(
        &source_root,
        MAX_PROMOTION_SNAPSHOT_ENTRIES,
        MAX_PROMOTION_SNAPSHOT_BYTES,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if source_tree
        .aliases_root(destination_tree)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?
        || source_tree
            .shares_regular_file_object_with(destination_tree)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "recovery source aliases the canonical destination".to_owned(),
        ));
    }
    let source_snapshot = snapshot_binding(
        &source_root,
        &source_tree,
        excluded_root_bindings(&source_root)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if source_snapshot != preview.source.snapshot {
        return Err(PromotionApplyError::RecoveryRequired(
            "isolation source bytes, namespace, or metadata changed after durable intent"
                .to_owned(),
        ));
    }
    let git_observation = observe_git_worktree(
        &source_root,
        &destination_root,
        &isolation.contract.branch_name,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if git_observation.binding != preview.source.git_worktree {
        return Err(PromotionApplyError::RecoveryRequired(
            "Git worktree binding changed after durable intent".to_owned(),
        ));
    }
    Ok(PreparedPromotion {
        derived_principal_id: Some(preview.source.linked_claim_principal_id.clone().ok_or(
            PromotionApplyError::RecoveryRequired(
                "stored preview lacks its derived principal".to_owned(),
            ),
        )?),
        preview,
        source_tree,
    })
}

fn validate_recovery_preview_identity(
    preview: &GovernedPromotionPreview,
    expected_preview_digest: &str,
) -> Result<(), PromotionApplyError> {
    if preview.preview_digest != expected_preview_digest
        || preview.status != forge_core_contracts::GovernedPromotionPreviewStatus::Reviewable
        || preview.apply_eligibility != PromotionApplyEligibility::EligibleLocalReversible
        || preview.authority != GovernedPromotionPreviewAuthority::ReadOnlyCandidateNoApplyAuthority
        || preview.canonical_mutation_performed
        || preview.forge_state_mutation_performed
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "stored preview is not the exact approved local-reversible candidate".to_owned(),
        ));
    }
    let mut canonical = preview.clone();
    canonical.preview_id = StableId(String::new());
    canonical.preview_digest.clear();
    canonical.observed_at_unix = 0;
    let actual = promotion_domain_digest("promotion.preview.v1", &canonical)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if actual != expected_preview_digest
        || preview.preview_id
            != StableId(format!(
                "promotion.preview.{}",
                expected_preview_digest.trim_start_matches("sha256:")
            ))
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "stored preview canonical identity is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_recovery_destination(
    destination_tree: &RetainedProjectTree,
    source_tree: &RetainedProjectTree,
    preview: &GovernedPromotionPreview,
) -> Result<(), PromotionApplyError> {
    let destination_files = inventory_files(destination_tree);
    let source_files = inventory_files(source_tree);
    if inventory_directories(destination_tree) != inventory_directories(source_tree)
        || destination_files.len() != source_files.len()
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "canonical destination namespace or directory metadata is incompatible with recovery"
                .to_owned(),
        ));
    }
    for source in &source_files {
        let current = destination_files
            .iter()
            .find(|candidate| candidate.relative_path == source.relative_path)
            .ok_or_else(|| {
                PromotionApplyError::RecoveryRequired(format!(
                    "{} disappeared from the canonical destination",
                    source.relative_path
                ))
            })?;
        if current.metadata_fingerprint != source.metadata_fingerprint {
            return Err(PromotionApplyError::RecoveryRequired(format!(
                "{} metadata differs from the approved source/destination",
                source.relative_path
            )));
        }
        let diff = preview
            .diff
            .iter()
            .find(|entry| entry.path.0 == source.relative_path);
        if let Some(diff) = diff {
            let old_digest = diff.before_content_digest.as_deref().ok_or_else(|| {
                PromotionApplyError::RecoveryRequired(format!(
                    "{} approved diff lacks old content",
                    source.relative_path
                ))
            })?;
            let new_digest = diff.after_content_digest.as_deref().ok_or_else(|| {
                PromotionApplyError::RecoveryRequired(format!(
                    "{} approved diff lacks new content",
                    source.relative_path
                ))
            })?;
            if current.content_digest != old_digest && current.content_digest != new_digest {
                return Err(PromotionApplyError::RecoveryRequired(format!(
                    "{} contains neither the recorded old nor exact new bytes",
                    source.relative_path
                )));
            }
        } else if current != source {
            return Err(PromotionApplyError::RecoveryRequired(format!(
                "{} changed outside the approved promotion write set",
                source.relative_path
            )));
        }
    }
    Ok(())
}

fn validate_pre_begin_destination_exact(
    binding: &WorkflowGovernanceProjectBinding,
    destination_tree: &RetainedProjectTree,
    preview: &GovernedPromotionPreview,
) -> Result<(), PromotionApplyError> {
    let destination_root =
        canonical_directory(&binding.project_root, "pre-begin recovery destination root")
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let current = snapshot_binding(
        &destination_root,
        destination_tree,
        excluded_root_bindings(&destination_root)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    if current != preview.destination.snapshot {
        return Err(PromotionApplyError::RecoveryRequired(
            "pre-begin recovery requires the canonical destination to exactly match the approved destination snapshot"
                .to_owned(),
        ));
    }
    Ok(())
}

fn promotion_provenance_from_intent(
    intent: &PromotionReplayIntent,
    preview: &GovernedPromotionPreview,
    effect: &ToolEffectContractDocument,
) -> Result<EffectExecutionProvenance, PromotionApplyError> {
    EffectExecutionProvenance::new(serde_json::json!({
        "kind": "governed_promotion_local_reversible_v1",
        "publication_scope": {
            "kind": "external_retained_project_tree_v1",
            "retained_capability_digest": &intent.publication_capability_digest,
            "canonical_root_digest": &preview.destination.snapshot.canonical_root_digest,
        },
        "preview": preview,
        "derived_principal_id": &intent.principal_id,
        "transaction_id": &intent.transaction_id,
        "effect": effect,
        "commit_digest": &intent.replay.commit_digest,
    }))
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))
}

fn recovery_execution_binding(intent: &PromotionReplayIntent) -> PromotionRecoveryExecutionBinding {
    PromotionRecoveryExecutionBinding {
        schema_version: PROMOTION_RECOVERY_EXECUTION_SCHEMA_VERSION.to_owned(),
        recovery_kind: if intent.schema_version == "governed_promotion_intent_v1" {
            PROMOTION_LEGACY_V1_PRE_BEGIN_RECOVERY_KIND
        } else {
            PROMOTION_PRE_BEGIN_RECOVERY_KIND
        }
        .to_owned(),
        durable_intent_digest: intent.replay.intent_digest.clone(),
        superseded_provenance_digest: intent.provenance_digest.clone(),
        superseded_publication_capability_digest: intent.publication_capability_digest.clone(),
    }
}

fn pre_begin_recovery_provenance(
    intent: &PromotionReplayIntent,
    preview: &GovernedPromotionPreview,
    effect: &ToolEffectContractDocument,
    publication_capability_digest: &str,
    recovery_execution: &PromotionRecoveryExecutionBinding,
) -> Result<EffectExecutionProvenance, PromotionApplyError> {
    EffectExecutionProvenance::new(serde_json::json!({
        "kind": "governed_promotion_recovery_execution_v1",
        "recovery": recovery_execution,
        "publication_scope": {
            "kind": "external_retained_project_tree_v1",
            "retained_capability_digest": publication_capability_digest,
            "canonical_root_digest": &preview.destination.snapshot.canonical_root_digest,
        },
        "preview": preview,
        "derived_principal_id": &intent.principal_id,
        "transaction_id": &intent.transaction_id,
        "effect": effect,
        "commit_digest": &intent.replay.commit_digest,
    }))
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))
}

fn recovery_publication_capability_from_wal(
    provenance: &EffectExecutionProvenance,
) -> Result<String, PromotionApplyError> {
    let document = &provenance.document;
    if document.get("kind").and_then(serde_json::Value::as_str)
        != Some("governed_promotion_recovery_execution_v1")
        || document
            .get("publication_scope")
            .and_then(|scope| scope.get("kind"))
            .and_then(serde_json::Value::as_str)
            != Some("external_retained_project_tree_v1")
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "effect WAL provenance is neither the durable original execution nor an admitted recovery execution"
                .to_owned(),
        ));
    }
    let digest = document
        .get("publication_scope")
        .and_then(|scope| scope.get("retained_capability_digest"))
        .and_then(serde_json::Value::as_str)
        .filter(|digest| is_sha256_digest(digest))
        .ok_or_else(|| {
            PromotionApplyError::RecoveryRequired(
                "effect WAL recovery provenance lacks a valid retained capability digest"
                    .to_owned(),
            )
        })?;
    Ok(digest.to_owned())
}

pub(super) fn apply_prepared_promotion_under_lock(
    binding: &WorkflowGovernanceProjectBinding,
    expected_preview_digest: &str,
    prepared: PreparedPromotion,
    destination_tree: &mut RetainedProjectTree,
    effect_lock: &EffectStoreLock,
    claim_guard: &forge_core_store::claim_wal::ExistingClaimProjectionGuard,
) -> Result<GovernedPromotionApplication, PromotionApplyError> {
    if prepared.preview.preview_digest != expected_preview_digest {
        return Err(PromotionApplyError::PreviewDigestMismatch {
            expected: expected_preview_digest.to_owned(),
            actual: prepared.preview.preview_digest,
        });
    }
    if prepared.preview.apply_eligibility != PromotionApplyEligibility::EligibleLocalReversible
        || prepared.preview.status
            != forge_core_contracts::GovernedPromotionPreviewStatus::Reviewable
    {
        return Err(PromotionApplyError::NotEligible(format!(
            "status={:?}, gaps={:?}",
            prepared.preview.status, prepared.preview.unresolved_gaps
        )));
    }
    let principal_id = prepared
        .derived_principal_id
        .clone()
        .ok_or(PromotionApplyError::MissingDerivedPrincipal)?;
    if prepared.preview.source.linked_claim_principal_id.as_ref() != Some(&principal_id) {
        return Err(PromotionApplyError::MissingDerivedPrincipal);
    }
    let (effect, payloads, applied_files) = promotion_effect_and_payloads(&prepared)?;
    let effect_id = effect.tool_effect_contract.id.clone();
    let transaction_id = StableId(format!(
        "promotion.tx.{}",
        expected_preview_digest.trim_start_matches("sha256:")
    ));
    let replay_key_hash = replay_nonce_key_hash(
        &principal_id,
        PROMOTION_REPLAY_AUDIENCE,
        expected_preview_digest,
    )
    .map_err(|error| PromotionApplyError::Payload(error.to_string()))?;
    let commit_digest = promotion_domain_digest(
        "promotion.commit.v1",
        &(
            expected_preview_digest,
            &transaction_id,
            &effect_id,
            &prepared.preview.diff_digest,
            &prepared.preview.write_set_digest,
        ),
    )
    .map_err(|error| PromotionApplyError::Payload(error.to_string()))?;
    let publication_capability_digest = destination_tree
        .exact_mutation_capability_digest()
        .map_err(|error| PromotionApplyError::UnsupportedEffect(error.to_string()))?;
    let provenance_document = serde_json::json!({
        "kind": "governed_promotion_local_reversible_v1",
        "publication_scope": {
            "kind": "external_retained_project_tree_v1",
            "retained_capability_digest": &publication_capability_digest,
            "canonical_root_digest": &prepared.preview.destination.snapshot.canonical_root_digest,
        },
        "preview": &prepared.preview,
        "derived_principal_id": &principal_id,
        "transaction_id": &transaction_id,
        "effect": &effect,
        "commit_digest": &commit_digest,
    });
    let provenance = EffectExecutionProvenance::new(provenance_document)
        .map_err(|error| PromotionApplyError::Payload(error.to_string()))?;
    let provisional_replay = PromotionReplayBinding {
        audience: PROMOTION_REPLAY_AUDIENCE.to_owned(),
        key_hash: replay_key_hash,
        intent_digest: String::new(),
        commit_digest,
        reservation_revision: 1,
    };
    let mut intent = PromotionReplayIntent {
        schema_version: "governed_promotion_intent_v2".to_owned(),
        expected_preview_digest: expected_preview_digest.to_owned(),
        principal_id: principal_id.clone(),
        transaction_id: transaction_id.clone(),
        effect_id: effect_id.clone(),
        replay: provisional_replay,
        provenance_digest: provenance.digest.clone(),
        publication_capability_digest,
        preview: Some(prepared.preview.clone()),
    };
    intent.replay.intent_digest = promotion_domain_digest("promotion.intent.v1", &intent)
        .map_err(|error| PromotionApplyError::Payload(error.to_string()))?;
    prepared
        .source_tree
        .revalidate()
        .map_err(|error| PromotionApplyError::Payload(error.to_string()))?;
    destination_tree
        .revalidate()
        .map_err(|error| PromotionApplyError::Preview(PromotionPreviewError::Snapshot(error)))?;
    claim_guard
        .revalidate()
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    effect_lock
        .validate_retained_lock_file()
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    ensure_preview_fresh(&prepared.preview, false)?;
    for file in &applied_files {
        let before = destination_tree
            .exact_regular_file_bytes(&file.path.0)
            .map_err(|error| PromotionApplyError::UnsupportedEffect(error.to_string()))?
            .ok_or_else(|| {
                PromotionApplyError::UnsupportedEffect(format!(
                    "{} is not an admitted existing regular file",
                    file.path.0
                ))
            })?;
        destination_tree
            .preflight_exact_regular_file_write(&file.path.0, &before)
            .map_err(|error| {
                PromotionApplyError::UnsupportedEffect(format!(
                    "{} cannot preserve exact admitted metadata before durable intent: {error}",
                    file.path.0
                ))
            })?;
    }

    let state_leaf = promotion_state_leaf_name(expected_preview_digest)?;
    let io = effect_lock
        .retained_store_io()
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    let intents = io
        .retain_subdirectory(Path::new("intents"))
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    let intent_bytes = serde_json_canonicalizer::to_vec(&intent)
        .map_err(|error| PromotionApplyError::Payload(error.to_string()))?;
    let mut intent_witness = intents
        .write_new_file_synced(
            Path::new(&state_leaf),
            &intent_bytes,
            PROMOTION_RECEIPT_MAX_BYTES,
        )
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    intent_witness
        .revalidate()
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?;
    debug_test_promotion_crash("after_intent");

    let reservation = reserve_replay_nonce_under_effect_lock(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        &principal_id,
        PROMOTION_REPLAY_AUDIENCE,
        expected_preview_digest,
        &intent.replay.intent_digest,
        &intent.replay.commit_digest,
    )
    .map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "durable promotion intent could not reserve replay authority: {error}"
        ))
    })?;
    if reservation.reservation.key_hash != intent.replay.key_hash
        || reservation.reservation.revision != intent.replay.reservation_revision
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "replay reservation differs from the durable promotion intent".to_owned(),
        ));
    }
    debug_test_promotion_crash("after_replay_reservation");
    let replay_binding = EffectReplayCommitBinding::new(
        reservation.reservation.key_hash.clone(),
        reservation.reservation.intent_digest.clone(),
        reservation.reservation.commit_digest.clone(),
        reservation.reservation.revision,
    );
    let replay_guard = acquire_replay_commit_guard(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        &principal_id,
        PROMOTION_REPLAY_AUDIENCE,
        expected_preview_digest,
        &intent.replay.intent_digest,
        &intent.replay.commit_digest,
        intent.replay.reservation_revision,
    )
    .map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "reserved promotion replay authority could not be retained: {error}"
        ))
    })?;

    claim_guard.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "claim projection changed after replay reservation: {error}"
        ))
    })?;
    prepared.source_tree.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "source changed after replay reservation: {error}"
        ))
    })?;
    destination_tree.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "destination changed after replay reservation: {error}"
        ))
    })?;
    ensure_preview_fresh(&prepared.preview, true)?;
    let effect_result = apply_existing_file_effect_transaction_to_retained_project_tree(
        &binding.state_root,
        destination_tree,
        replay_guard.effect_lock(),
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        &effect,
        &payloads,
        PROMOTION_EFFECT_WAL_RELATIVE_PATH,
        transaction_id.0.clone(),
        provenance.clone(),
        replay_binding,
    );
    if effect_result.status != EffectApplicationStatus::Applied {
        return Err(PromotionApplyError::RecoveryRequired(format!(
            "reserved promotion effect did not complete: status={:?}, diagnostics={:?}",
            effect_result.status, effect_result.diagnostics
        )));
    }
    let replay_result = replay_guard.consume().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "promotion effect committed but replay consume failed: {error}"
        ))
    })?;
    debug_test_promotion_crash("after_replay_consume");
    append_effect_replay_completion_under_lock(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        PROMOTION_EFFECT_WAL_RELATIVE_PATH,
        &transaction_id.0,
        &effect_id,
        &EffectReplayCommitBinding::new(
            intent.replay.key_hash.clone(),
            intent.replay.intent_digest.clone(),
            intent.replay.commit_digest.clone(),
            intent.replay.reservation_revision,
        ),
        &replay_result,
        false,
    )
    .map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "promotion replay consumed but effect completion marker failed: {error}"
        ))
    })?;

    let committed_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?
        .as_secs();
    destination_tree.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "exact retained canonical readback failed: {error}"
        ))
    })?;
    verify_logical_result_matches_source(destination_tree, &prepared.source_tree)
        .map_err(PromotionApplyError::RecoveryRequired)?;
    let result_root = canonical_directory(&binding.project_root, "result root")
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let result_snapshot = snapshot_binding(
        &result_root,
        destination_tree,
        excluded_root_bindings(&result_root)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let result_snapshot_digest =
        promotion_domain_digest("promotion.result_snapshot.v1", &result_snapshot)
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    debug_test_promotion_crash("after_readback");
    let mut receipt = GovernedPromotionReceipt {
        schema_version: GOVERNED_PROMOTION_RECEIPT_SCHEMA_VERSION.to_owned(),
        receipt_digest: String::new(),
        committed_at_unix,
        preview: prepared.preview,
        derived_principal_id: principal_id,
        transaction_id,
        effect_id,
        provenance_digest: provenance.digest,
        publication_capability_digest: intent.publication_capability_digest.clone(),
        replay: intent.replay,
        recovery_execution: None,
        applied_files,
        result_snapshot,
        result_snapshot_digest,
        readback_verified: true,
    };
    receipt.receipt_digest = promotion_receipt_digest(&receipt)?;
    let receipt_bytes = serde_json_canonicalizer::to_vec(&receipt)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let receipts = io
        .retain_subdirectory(Path::new("receipts"))
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let mut receipt_witness = receipts
        .write_new_file_synced(
            Path::new(&state_leaf),
            &receipt_bytes,
            PROMOTION_RECEIPT_MAX_BYTES,
        )
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    receipt_witness
        .revalidate()
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    debug_test_promotion_crash("after_receipt");
    let persisted: GovernedPromotionReceipt =
        serde_json::from_slice(receipt_witness.raw_bytes())
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    verify_promotion_receipt(&persisted, expected_preview_digest)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    verify_consumed_replay_binding_under_lock(binding, effect_lock, &persisted)?;
    verify_committed_receipt_readback(binding, &persisted)
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    Ok(GovernedPromotionApplication {
        status: GovernedPromotionApplyStatus::Applied,
        canonical_mutation_performed: true,
        receipt: persisted,
    })
}

fn verify_logical_result_matches_source(
    result_tree: &RetainedProjectTree,
    source_tree: &RetainedProjectTree,
) -> Result<(), String> {
    let mut source = inventory_files(source_tree)
        .into_iter()
        .map(|file| (file.relative_path, file.content_digest, file.byte_length))
        .collect::<Vec<_>>();
    source.sort();
    let result = inventory_files(result_tree);
    let mut logical_result = result
        .into_iter()
        .map(|file| (file.relative_path, file.content_digest, file.byte_length))
        .collect::<Vec<_>>();
    logical_result.sort();
    if logical_result != source {
        let source_paths = source
            .iter()
            .map(|(path, _, _)| path.as_str())
            .collect::<Vec<_>>();
        let result_paths = logical_result
            .iter()
            .map(|(path, _, _)| path.as_str())
            .collect::<Vec<_>>();
        return Err(format!(
            "canonical logical readback differs from the predicted exact result: source_paths={source_paths:?}, result_paths={result_paths:?}"
        ));
    }
    Ok(())
}

fn promotion_effect_and_payloads(
    prepared: &PreparedPromotion,
) -> Result<
    (
        ToolEffectContractDocument,
        Vec<EffectApplicationPayload>,
        Vec<PromotionAppliedFileBinding>,
    ),
    PromotionApplyError,
> {
    let principal = prepared
        .derived_principal_id
        .as_ref()
        .ok_or(PromotionApplyError::MissingDerivedPrincipal)?;
    let mut payloads = Vec::new();
    let mut applied = Vec::new();
    for entry in &prepared.preview.diff {
        if entry.effect != PromotionDiffEffect::WriteRegularFile
            || entry.destructive
            || entry.before_metadata_fingerprint != entry.after_metadata_fingerprint
        {
            return Err(PromotionApplyError::UnsupportedEffect(format!(
                "{} is not one metadata-stable write to an existing regular file",
                entry.path.0
            )));
        }
        let before = entry.before_content_digest.clone().ok_or_else(|| {
            PromotionApplyError::Payload(format!("{} lacks before digest", entry.path.0))
        })?;
        let after = entry.after_content_digest.clone().ok_or_else(|| {
            PromotionApplyError::Payload(format!("{} lacks after digest", entry.path.0))
        })?;
        let content = prepared
            .source_tree
            .exact_regular_file_bytes(&entry.path.0)
            .map_err(|error| PromotionApplyError::Payload(error.to_string()))?
            .ok_or_else(|| {
                PromotionApplyError::Payload(format!("{} missing in retained source", entry.path.0))
            })?;
        if sha256_content_hash(&content) != after {
            return Err(PromotionApplyError::Payload(format!(
                "{} retained source digest differs from preview",
                entry.path.0
            )));
        }
        payloads.push(EffectApplicationPayload {
            target_ref: entry.path.0.clone(),
            content_hash: after.clone(),
            content,
        });
        applied.push(PromotionAppliedFileBinding {
            path: entry.path.clone(),
            before_content_digest: before,
            before_byte_length: entry.before_byte_length.unwrap_or_default(),
            after_content_digest: after,
            after_byte_length: entry.after_byte_length.unwrap_or_default(),
        });
    }
    let effect = promotion_effect_contract(&prepared.preview, principal)?;
    Ok((effect, payloads, applied))
}

fn promotion_effect_contract(
    preview: &GovernedPromotionPreview,
    principal: &PrincipalId,
) -> Result<ToolEffectContractDocument, PromotionApplyError> {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    for entry in &preview.diff {
        if entry.effect != PromotionDiffEffect::WriteRegularFile
            || entry.destructive
            || entry.before_metadata_fingerprint != entry.after_metadata_fingerprint
        {
            return Err(PromotionApplyError::UnsupportedEffect(format!(
                "{} is not one metadata-stable write to an existing regular file",
                entry.path.0
            )));
        }
        let before = entry.before_content_digest.clone().ok_or_else(|| {
            PromotionApplyError::Payload(format!("{} lacks before digest", entry.path.0))
        })?;
        reads.push(EffectRead {
            target_kind: EffectTargetKind::FilePath,
            reference: entry.path.0.clone(),
            expected_hash: Some(before.clone()),
            expected_version: None,
            required_for_plan: true,
        });
        writes.push(EffectWrite {
            target_kind: EffectTargetKind::FilePath,
            reference: entry.path.0.clone(),
            access_mode: AccessMode::Write,
            expected_hash: Some(before),
            expected_version: None,
            destructive: false,
        });
    }
    if writes.is_empty() {
        return Err(PromotionApplyError::UnsupportedEffect(
            "apply requires at least one exact regular-file write".to_owned(),
        ));
    }
    let effect_id = StableId(format!(
        "promotion.effect.{}",
        preview.preview_digest.trim_start_matches("sha256:")
    ));
    let effect = ToolEffectContractDocument {
        schema_version: "0.1".to_owned(),
        tool_effect_contract: ToolEffectContract {
            id: effect_id,
            contract_ref: RepoPath(format!(
                "promotion/derived/{}.json",
                preview.preview_digest.trim_start_matches("sha256:")
            )),
            effect_kind: EffectKind::OperationTransaction,
            operation_ref: preview.objective.objective_id.clone(),
            actor: EffectActor {
                agent_id: preview.source.agent_id.clone(),
                role: ActorRole::Worker,
            },
            read_set: reads,
            write_set: writes,
            conflict_detection: ConflictDetection {
                check_against: StableId("promotion-current-destination-and-claims".to_owned()),
                granularity: StableId("normalized-file-path".to_owned()),
                conflict_codes: vec![
                    ConflictCode::ReadTargetChanged,
                    ConflictCode::WriteTargetChanged,
                    ConflictCode::WriteTargetClaimed,
                    ConflictCode::OverlappingWriteSet,
                ],
                policy: ConflictPolicy::Block,
            },
            notification: EffectNotification {
                required: false,
                recipients: vec![StableId(principal.0.clone())],
                request_contract_ref: None,
            },
            repair: EffectRepair {
                strategy: RepairStrategy::None,
                automatic_repair_allowed: false,
                inverse_operation_ref: None,
                stop_if_inverse_missing: false,
                inverse: InverseMetadata {
                    kind: InverseKind::None,
                    source: InverseSource::Unavailable,
                    reference: None,
                    input_mapping_refs: Vec::new(),
                    validation_gate_refs: Vec::new(),
                    review_required: false,
                },
            },
        },
    };
    Ok(effect)
}

fn promotion_state_leaf_name(expected_preview_digest: &str) -> Result<String, PromotionApplyError> {
    let Some(hex) = expected_preview_digest.strip_prefix("sha256:") else {
        return Err(PromotionApplyError::InvalidExpectedPreviewDigest);
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PromotionApplyError::InvalidExpectedPreviewDigest);
    }
    Ok(format!("{}.json", hex.to_ascii_lowercase()))
}

fn ensure_preview_fresh(
    preview: &GovernedPromotionPreview,
    reservation_durable: bool,
) -> Result<(), PromotionApplyError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| PromotionApplyError::Store(error.to_string()))?
        .as_secs();
    if preview
        .valid_through_unix
        .is_some_and(|valid_through| now > valid_through)
    {
        let reason = "claim or cooperative evidence freshness expired before first write";
        return if reservation_durable {
            Err(PromotionApplyError::RecoveryRequired(reason.to_owned()))
        } else {
            Err(PromotionApplyError::NotEligible(reason.to_owned()))
        };
    }
    Ok(())
}

fn promotion_receipt_digest(
    receipt: &GovernedPromotionReceipt,
) -> Result<String, PromotionApplyError> {
    let mut canonical = receipt.clone();
    canonical.receipt_digest.clear();
    promotion_domain_digest("promotion.receipt.v1", &canonical)
        .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))
}

fn verify_promotion_receipt(
    receipt: &GovernedPromotionReceipt,
    expected_preview_digest: &str,
) -> Result<(), PromotionApplyError> {
    if receipt.schema_version != GOVERNED_PROMOTION_RECEIPT_SCHEMA_VERSION
        || receipt.preview.preview_digest != expected_preview_digest
        || !receipt.readback_verified
    {
        return Err(PromotionApplyError::ReceiptInvalid(
            "schema, preview identity, or readback flag mismatch".to_owned(),
        ));
    }
    let preview = &receipt.preview;
    if preview.status != forge_core_contracts::GovernedPromotionPreviewStatus::Reviewable
        || preview.apply_eligibility != PromotionApplyEligibility::EligibleLocalReversible
        || preview.authority != GovernedPromotionPreviewAuthority::ReadOnlyCandidateNoApplyAuthority
        || preview.canonical_mutation_performed
        || preview.forge_state_mutation_performed
    {
        return Err(PromotionApplyError::ReceiptInvalid(
            "receipt preview is not an honest reviewable read-only candidate".to_owned(),
        ));
    }
    if preview.source.linked_claim_principal_id.as_ref() != Some(&receipt.derived_principal_id) {
        return Err(PromotionApplyError::ReceiptInvalid(
            "derived principal differs from the exact linked claim".to_owned(),
        ));
    }
    let mut canonical_preview = preview.clone();
    canonical_preview.preview_id = StableId(String::new());
    canonical_preview.preview_digest.clear();
    canonical_preview.observed_at_unix = 0;
    let actual_preview_digest = promotion_domain_digest("promotion.preview.v1", &canonical_preview)
        .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    let actual_preview_id = StableId(format!(
        "promotion.preview.{}",
        actual_preview_digest.trim_start_matches("sha256:")
    ));
    if preview.preview_digest != actual_preview_digest || preview.preview_id != actual_preview_id {
        return Err(PromotionApplyError::ReceiptInvalid(
            "embedded preview identity is not derived from its canonical semantics".to_owned(),
        ));
    }
    let digest_hex = expected_preview_digest
        .strip_prefix("sha256:")
        .ok_or(PromotionApplyError::InvalidExpectedPreviewDigest)?;
    let expected_tx = StableId(format!("promotion.tx.{digest_hex}"));
    let expected_effect_id = StableId(format!("promotion.effect.{digest_hex}"));
    if receipt.transaction_id != expected_tx || receipt.effect_id != expected_effect_id {
        return Err(PromotionApplyError::ReceiptInvalid(
            "transaction or effect identity is not derived from the preview".to_owned(),
        ));
    }
    for digest in [
        &preview.preview_digest,
        &preview.diff_digest,
        &preview.write_set_digest,
        &preview.predicted_result_regular_file_set_digest,
        &receipt.provenance_digest,
        &receipt.publication_capability_digest,
        &receipt.replay.key_hash,
        &receipt.replay.intent_digest,
        &receipt.replay.commit_digest,
        &receipt.result_snapshot_digest,
        &receipt.receipt_digest,
    ] {
        if !is_sha256_digest(digest) {
            return Err(PromotionApplyError::ReceiptInvalid(
                "receipt contains a malformed content digest".to_owned(),
            ));
        }
    }
    if let Some(recovery) = &receipt.recovery_execution {
        if recovery.schema_version != PROMOTION_RECOVERY_EXECUTION_SCHEMA_VERSION
            || !matches!(
                recovery.recovery_kind.as_str(),
                PROMOTION_PRE_BEGIN_RECOVERY_KIND | PROMOTION_LEGACY_V1_PRE_BEGIN_RECOVERY_KIND
            )
            || recovery.durable_intent_digest != receipt.replay.intent_digest
            || !is_sha256_digest(&recovery.superseded_provenance_digest)
            || !is_sha256_digest(&recovery.superseded_publication_capability_digest)
        {
            return Err(PromotionApplyError::ReceiptInvalid(
                "receipt recovery execution binding is malformed or contradicts replay authority"
                    .to_owned(),
            ));
        }
    }
    if receipt.replay.audience != PROMOTION_REPLAY_AUDIENCE
        || receipt.replay.reservation_revision != 1
    {
        return Err(PromotionApplyError::ReceiptInvalid(
            "replay audience or reservation revision is invalid".to_owned(),
        ));
    }
    let expected_key = replay_nonce_key_hash(
        &receipt.derived_principal_id,
        PROMOTION_REPLAY_AUDIENCE,
        expected_preview_digest,
    )
    .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    let expected_commit = promotion_domain_digest(
        "promotion.commit.v1",
        &(
            expected_preview_digest,
            &receipt.transaction_id,
            &receipt.effect_id,
            &preview.diff_digest,
            &preview.write_set_digest,
        ),
    )
    .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    if receipt.replay.key_hash != expected_key || receipt.replay.commit_digest != expected_commit {
        return Err(PromotionApplyError::ReceiptInvalid(
            "replay key or commit digest differs from receipt semantics".to_owned(),
        ));
    }
    let expected_effect_contract =
        promotion_effect_contract(preview, &receipt.derived_principal_id).map_err(|error| {
            PromotionApplyError::ReceiptInvalid(format!(
                "receipt preview cannot derive its claimed effect: {error}"
            ))
        })?;
    if expected_effect_contract.tool_effect_contract.id != receipt.effect_id {
        return Err(PromotionApplyError::ReceiptInvalid(
            "derived effect contract identity differs from the receipt".to_owned(),
        ));
    }
    let original_publication_capability_digest = receipt
        .recovery_execution
        .as_ref()
        .map_or(&receipt.publication_capability_digest, |recovery| {
            &recovery.superseded_publication_capability_digest
        });
    let original_provenance_digest = receipt
        .recovery_execution
        .as_ref()
        .map_or(&receipt.provenance_digest, |recovery| {
            &recovery.superseded_provenance_digest
        });
    let expected_original_provenance = EffectExecutionProvenance::new(serde_json::json!({
        "kind": "governed_promotion_local_reversible_v1",
        "publication_scope": {
            "kind": "external_retained_project_tree_v1",
            "retained_capability_digest": original_publication_capability_digest,
            "canonical_root_digest": &preview.destination.snapshot.canonical_root_digest,
        },
        "preview": preview,
        "derived_principal_id": &receipt.derived_principal_id,
        "transaction_id": &receipt.transaction_id,
        "effect": &expected_effect_contract,
        "commit_digest": &expected_commit,
    }))
    .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    let original_provenance_reconstructed =
        original_provenance_digest == &expected_original_provenance.digest;
    let disclosed_legacy_v1_recovery =
        receipt.recovery_execution.as_ref().is_some_and(|recovery| {
            recovery.recovery_kind == PROMOTION_LEGACY_V1_PRE_BEGIN_RECOVERY_KIND
        });
    if !original_provenance_reconstructed && !disclosed_legacy_v1_recovery {
        return Err(PromotionApplyError::ReceiptInvalid(
            "original execution provenance digest differs from the approved receipt semantics"
                .to_owned(),
        ));
    }
    let mut replay_without_intent = receipt.replay.clone();
    replay_without_intent.intent_digest.clear();
    let mut intent = PromotionReplayIntent {
        schema_version: "governed_promotion_intent_v2".to_owned(),
        expected_preview_digest: expected_preview_digest.to_owned(),
        principal_id: receipt.derived_principal_id.clone(),
        transaction_id: receipt.transaction_id.clone(),
        effect_id: receipt.effect_id.clone(),
        replay: replay_without_intent,
        provenance_digest: original_provenance_digest.to_owned(),
        publication_capability_digest: original_publication_capability_digest.to_owned(),
        preview: Some(receipt.preview.clone()),
    };
    let mut expected_intent = promotion_domain_digest("promotion.intent.v1", &intent)
        .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    if receipt.replay.intent_digest != expected_intent {
        intent.schema_version = "governed_promotion_intent_v1".to_owned();
        intent.preview = None;
        expected_intent = promotion_domain_digest("promotion.intent.v1", &intent)
            .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    }
    if receipt.replay.intent_digest != expected_intent {
        return Err(PromotionApplyError::ReceiptInvalid(
            "replay intent digest differs from receipt semantics".to_owned(),
        ));
    }
    let legacy_v1_fresh_execution = intent.schema_version == "governed_promotion_intent_v1"
        && receipt.recovery_execution.is_some();
    if !original_provenance_reconstructed && !legacy_v1_fresh_execution {
        return Err(PromotionApplyError::ReceiptInvalid(
            "original execution provenance digest differs from the approved receipt semantics"
                .to_owned(),
        ));
    }
    if let Some(recovery) = &receipt.recovery_execution {
        let expected_kind = if legacy_v1_fresh_execution {
            PROMOTION_LEGACY_V1_PRE_BEGIN_RECOVERY_KIND
        } else {
            PROMOTION_PRE_BEGIN_RECOVERY_KIND
        };
        if recovery.recovery_kind != expected_kind {
            return Err(PromotionApplyError::ReceiptInvalid(
                "recovery kind contradicts the self-digested original intent schema".to_owned(),
            ));
        }
        let expected_recovery = pre_begin_recovery_provenance(
            &intent,
            preview,
            &expected_effect_contract,
            &receipt.publication_capability_digest,
            recovery,
        )
        .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
        if receipt.provenance_digest != expected_recovery.digest {
            return Err(PromotionApplyError::ReceiptInvalid(
                "actual recovery execution provenance differs from its durable original-intent link"
                    .to_owned(),
            ));
        }
    } else if receipt.provenance_digest != expected_original_provenance.digest {
        return Err(PromotionApplyError::ReceiptInvalid(
            "execution provenance digest differs from the receipt semantics".to_owned(),
        ));
    }
    let expected_applied = preview
        .diff
        .iter()
        .map(|entry| {
            if entry.effect != PromotionDiffEffect::WriteRegularFile
                || entry.destructive
                || entry.before_metadata_fingerprint != entry.after_metadata_fingerprint
            {
                return Err(PromotionApplyError::ReceiptInvalid(
                    "receipt preview contains a non-admitted apply effect".to_owned(),
                ));
            }
            Ok(PromotionAppliedFileBinding {
                path: entry.path.clone(),
                before_content_digest: entry.before_content_digest.clone().ok_or_else(|| {
                    PromotionApplyError::ReceiptInvalid(
                        "receipt preview write lacks before digest".to_owned(),
                    )
                })?,
                before_byte_length: entry.before_byte_length.ok_or_else(|| {
                    PromotionApplyError::ReceiptInvalid(
                        "receipt preview write lacks before length".to_owned(),
                    )
                })?,
                after_content_digest: entry.after_content_digest.clone().ok_or_else(|| {
                    PromotionApplyError::ReceiptInvalid(
                        "receipt preview write lacks after digest".to_owned(),
                    )
                })?,
                after_byte_length: entry.after_byte_length.ok_or_else(|| {
                    PromotionApplyError::ReceiptInvalid(
                        "receipt preview write lacks after length".to_owned(),
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if receipt.applied_files != expected_applied
        || preview.write_set
            != receipt
                .applied_files
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>()
    {
        return Err(PromotionApplyError::ReceiptInvalid(
            "receipt applied-file set differs from the exact preview diff".to_owned(),
        ));
    }
    let source = &preview.source.snapshot;
    let result = &receipt.result_snapshot;
    let mut snapshot_mismatches = Vec::new();
    if result.canonical_root != preview.destination.snapshot.canonical_root {
        snapshot_mismatches.push("canonical_root");
    }
    if result.canonical_root_digest != preview.destination.snapshot.canonical_root_digest {
        snapshot_mismatches.push("canonical_root_digest");
    }
    if result.excluded_roots != preview.destination.snapshot.excluded_roots {
        snapshot_mismatches.push("excluded_roots");
    }
    if result.snapshot_digest != source.snapshot_digest {
        snapshot_mismatches.push("snapshot_digest");
    }
    if result.retained_tree_digest != source.retained_tree_digest {
        snapshot_mismatches.push("retained_tree_digest");
    }
    if result.regular_file_set_digest != source.regular_file_set_digest {
        snapshot_mismatches.push("regular_file_set_digest");
    }
    if result.file_count != source.file_count {
        snapshot_mismatches.push("file_count");
    }
    if result.directory_count != source.directory_count {
        snapshot_mismatches.push("directory_count");
    }
    if result.total_regular_file_bytes != source.total_regular_file_bytes {
        snapshot_mismatches.push("total_regular_file_bytes");
    }
    if !snapshot_mismatches.is_empty() {
        return Err(PromotionApplyError::ReceiptInvalid(format!(
            "receipt result snapshot differs from the exact predicted source result: {}",
            snapshot_mismatches.join(", ")
        )));
    }
    let expected_result_digest =
        promotion_domain_digest("promotion.result_snapshot.v1", &receipt.result_snapshot)
            .map_err(|error| PromotionApplyError::ReceiptInvalid(error.to_string()))?;
    if receipt.result_snapshot_digest != expected_result_digest
        || receipt.committed_at_unix < preview.observed_at_unix
    {
        return Err(PromotionApplyError::ReceiptInvalid(
            "result snapshot digest or commit time is invalid".to_owned(),
        ));
    }
    let actual = promotion_receipt_digest(receipt)?;
    if actual != receipt.receipt_digest {
        return Err(PromotionApplyError::ReceiptInvalid(format!(
            "self digest mismatch: expected {}, actual {actual}",
            receipt.receipt_digest
        )));
    }
    Ok(())
}

fn verify_consumed_replay_binding_under_lock(
    binding: &WorkflowGovernanceProjectBinding,
    effect_lock: &EffectStoreLock,
    receipt: &GovernedPromotionReceipt,
) -> Result<(), PromotionApplyError> {
    let consumed = verify_consumed_replay_key_hash_under_effect_lock(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        &receipt.replay.key_hash,
        &receipt.replay.intent_digest,
        &receipt.replay.commit_digest,
        receipt.replay.reservation_revision,
    )
    .map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "committed receipt replay binding is not durably consumed: {error}"
        ))
    })?;
    if consumed.key_hash != receipt.replay.key_hash
        || consumed.intent_digest != receipt.replay.intent_digest
        || consumed.commit_digest != receipt.replay.commit_digest
        || consumed.revision != receipt.replay.reservation_revision.saturating_add(1)
    {
        return Err(PromotionApplyError::RecoveryRequired(
            "consumed replay authority differs from the committed receipt".to_owned(),
        ));
    }
    Ok(())
}

fn verify_committed_receipt_readback(
    binding: &WorkflowGovernanceProjectBinding,
    receipt: &GovernedPromotionReceipt,
) -> Result<(), PromotionApplyError> {
    let tree = RetainedProjectTree::capture(
        &binding.project_root,
        MAX_PROMOTION_SNAPSHOT_ENTRIES,
        MAX_PROMOTION_SNAPSHOT_BYTES,
    )
    .map_err(|error| PromotionApplyError::Readback(error.to_string()))?;
    let root = canonical_directory(&binding.project_root, "receipt readback root")
        .map_err(|error| PromotionApplyError::Readback(error.to_string()))?;
    let actual = snapshot_binding(
        &root,
        &tree,
        excluded_root_bindings(&root)
            .map_err(|error| PromotionApplyError::Readback(error.to_string()))?,
    )
    .map_err(|error| PromotionApplyError::Readback(error.to_string()))?;
    if actual != receipt.result_snapshot {
        return Err(PromotionApplyError::Readback(
            "canonical project bytes, namespace, or admitted metadata no longer match the committed receipt"
                .to_owned(),
        ));
    }
    tree.revalidate()
        .map_err(|error| PromotionApplyError::Readback(error.to_string()))
}

fn is_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(debug_assertions)]
fn debug_test_promotion_crash(point: &str) {
    if std::env::var("FORGE_TEST_PROMOTION_CRASH_AT").as_deref() == Ok(point) {
        std::process::exit(86);
    }
}

#[cfg(not(debug_assertions))]
fn debug_test_promotion_crash(_point: &str) {}

pub(super) fn inspect_replacement_workspace(
    binding: &WorkflowGovernanceProjectBinding,
    readiness_profile: WorkflowReadinessProfile,
    guidance: &WorkflowGovernanceGuidance,
    now: u64,
) -> ReplacementWorkspaceInspection {
    let mut result = ReplacementWorkspaceInspection::default();
    let registry_root = binding.state_root.join("contracts").join("isolations");
    let selections = match fs::symlink_metadata(&registry_root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::IsolationRegistryInvalid,
                true,
                format!("The isolation registry could not be read without changing it: {error}"),
                None,
            ));
            return result;
        }
        Ok(_) => match load_isolation_registry(&binding.state_root) {
            Ok(selections) => selections,
            Err(error) => {
                result.gaps.push(replacement_gap(
                    ReplacementWorkspaceGapCode::IsolationRegistryInvalid,
                    true,
                    format!("The isolation registry is not trustworthy: {error}"),
                    None,
                ));
                return result;
            }
        },
    };

    let mut seen_ids = BTreeSet::new();
    for isolation in &selections {
        if !seen_ids.insert(isolation.contract.id.clone()) {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::IsolationRegistryInvalid,
                true,
                format!(
                    "Isolation id {} appears in more than one durable contract.",
                    isolation.contract.id.0
                ),
                Some(isolation.contract.id.clone()),
            ));
        }
    }
    for (index, isolation) in selections.iter().enumerate() {
        let previous = selections[..index]
            .iter()
            .map(|candidate| &candidate.contract)
            .collect::<Vec<_>>();
        if let Err(error) = detect_isolation_conflict(&isolation.contract, &previous) {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::IsolationConflict,
                true,
                format!(
                    "Isolation {} conflicts with another live isolation: {error}",
                    isolation.contract.id.0
                ),
                Some(isolation.contract.id.clone()),
            ));
        }
    }

    let destination = canonical_directory(&binding.project_root, "replacement destination root");
    for isolation in &selections {
        let mut gap_codes = result
            .gaps
            .iter()
            .filter(|gap| gap.isolation_id.as_ref() == Some(&isolation.contract.id))
            .map(|gap| gap.code)
            .collect::<Vec<_>>();
        let declared = match declared_worktree_candidate(
            &binding.project_root,
            &isolation.contract.worktree_path,
        ) {
            Ok(path) => path,
            Err(error) => {
                gap_codes.push(ReplacementWorkspaceGapCode::GitWorktreeMismatch);
                result.gaps.push(replacement_gap(
                    ReplacementWorkspaceGapCode::GitWorktreeMismatch,
                    matches!(
                        isolation.contract.status,
                        IsolationStatus::Active | IsolationStatus::Merging
                    ),
                    format!(
                        "Isolation {} declares an invalid worktree path: {error}",
                        isolation.contract.id.0
                    ),
                    Some(isolation.contract.id.clone()),
                ));
                binding
                    .project_root
                    .join(&isolation.contract.worktree_path.0)
            }
        };
        let declared_text = declared.display().to_string();
        let live = matches!(
            isolation.contract.status,
            IsolationStatus::Active | IsolationStatus::Merging
        );
        let (validation, git) = match fs::symlink_metadata(&declared) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match isolation.contract.status {
                    IsolationStatus::Proposed => {
                        (ReplacementIsolationValidation::ProposedNotCreated, None)
                    }
                    IsolationStatus::Merged | IsolationStatus::Abandoned => {
                        (ReplacementIsolationValidation::RetiredWorktreeAbsent, None)
                    }
                    IsolationStatus::Active | IsolationStatus::Merging => {
                        gap_codes.push(ReplacementWorkspaceGapCode::WorktreeMissing);
                        result.gaps.push(replacement_gap(
                            ReplacementWorkspaceGapCode::WorktreeMissing,
                            true,
                            format!(
                                "Isolation {} is live, but its declared worktree is missing.",
                                isolation.contract.id.0
                            ),
                            Some(isolation.contract.id.clone()),
                        ));
                        (ReplacementIsolationValidation::Missing, None)
                    }
                }
            }
            Err(error) => {
                gap_codes.push(ReplacementWorkspaceGapCode::GitWorktreeMismatch);
                result.gaps.push(replacement_gap(
                    ReplacementWorkspaceGapCode::GitWorktreeMismatch,
                    live,
                    format!(
                        "Isolation {} worktree could not be inspected: {error}",
                        isolation.contract.id.0
                    ),
                    Some(isolation.contract.id.clone()),
                ));
                (ReplacementIsolationValidation::Mismatched, None)
            }
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                gap_codes.push(ReplacementWorkspaceGapCode::GitWorktreeMismatch);
                result.gaps.push(replacement_gap(
                    ReplacementWorkspaceGapCode::GitWorktreeMismatch,
                    live,
                    format!(
                        "Isolation {} worktree is not a no-follow directory.",
                        isolation.contract.id.0
                    ),
                    Some(isolation.contract.id.clone()),
                ));
                (ReplacementIsolationValidation::Mismatched, None)
            }
            Ok(_) => {
                let observed = match &destination {
                    Ok(destination) => canonical_directory(
                        &declared,
                        "replacement isolation worktree",
                    )
                    .and_then(|source| {
                        observe_git_worktree(&source, destination, &isolation.contract.branch_name)
                    }),
                    Err(error) => Err(PromotionPreviewError::GitWorktree(format!(
                        "canonical destination could not be retained: {error}"
                    ))),
                };
                match observed {
                    Ok(observed) => (
                        ReplacementIsolationValidation::Valid,
                        Some(observed.binding),
                    ),
                    Err(error) => {
                        gap_codes.push(ReplacementWorkspaceGapCode::GitWorktreeMismatch);
                        result.gaps.push(replacement_gap(
                            ReplacementWorkspaceGapCode::GitWorktreeMismatch,
                            live,
                            format!(
                                "Isolation {} is not the declared branch/worktree in the canonical repository: {error}",
                                isolation.contract.id.0
                            ),
                            Some(isolation.contract.id.clone()),
                        ));
                        (ReplacementIsolationValidation::Mismatched, None)
                    }
                }
            }
        };
        gap_codes.sort();
        gap_codes.dedup();
        result.isolations.push(ReplacementIsolationInspection {
            contract_path: isolation.relative_path.clone(),
            contract_digest: isolation.raw_digest.clone(),
            contract: isolation.contract.clone(),
            declared_worktree: declared_text,
            validation,
            git,
            gap_codes,
        });
    }

    inspect_replacement_promotions(
        binding,
        readiness_profile,
        guidance,
        now,
        &selections,
        &mut result,
    );
    result.isolations.sort_by(|left, right| {
        left.contract
            .id
            .0
            .cmp(&right.contract.id.0)
            .then_with(|| left.contract_path.cmp(&right.contract_path))
    });
    result.promotions.sort_by(|left, right| {
        left.isolation_id
            .0
            .cmp(&right.isolation_id.0)
            .then_with(|| left.preview_digest.cmp(&right.preview_digest))
            .then_with(|| {
                replacement_promotion_status_rank(left.status)
                    .cmp(&replacement_promotion_status_rank(right.status))
            })
    });
    result.gaps.sort_by(|left, right| {
        left.isolation_id
            .cmp(&right.isolation_id)
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.summary.cmp(&right.summary))
    });
    result
}

fn replacement_gap(
    code: ReplacementWorkspaceGapCode,
    blocking: bool,
    summary: String,
    isolation_id: Option<StableId>,
) -> ReplacementWorkspaceGap {
    ReplacementWorkspaceGap {
        code,
        blocking,
        summary,
        isolation_id,
    }
}

const fn replacement_promotion_status_rank(status: ReplacementPromotionStatus) -> u8 {
    match status {
        ReplacementPromotionStatus::NotStarted => 0,
        ReplacementPromotionStatus::Recoverable => 1,
        ReplacementPromotionStatus::Completed => 2,
        ReplacementPromotionStatus::BlockedCorrupt => 3,
    }
}

fn inspect_replacement_promotions(
    binding: &WorkflowGovernanceProjectBinding,
    readiness_profile: WorkflowReadinessProfile,
    guidance: &WorkflowGovernanceGuidance,
    now: u64,
    selections: &[IsolationSelection],
    result: &mut ReplacementWorkspaceInspection,
) {
    let promotion_root = binding.state_root.join("promotion");
    match fs::symlink_metadata(&promotion_root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            add_not_started_promotions(selections, &BTreeSet::new(), result);
            return;
        }
        Err(error) => {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::PromotionStateInvalid,
                true,
                format!("Promotion state could not be inspected: {error}"),
                None,
            ));
            return;
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::PromotionStateInvalid,
                true,
                "Promotion state is not a no-follow directory.".to_owned(),
                None,
            ));
            return;
        }
        Ok(_) => {}
    }
    let effect_lock = match acquire_existing_effect_store_lock(
        &binding.state_root,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
    ) {
        Ok(lock) => lock,
        Err(error) => {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::PromotionStateInvalid,
                true,
                format!(
                    "Promotion state exists, but its existing lock could not be retained: {error}"
                ),
                None,
            ));
            return;
        }
    };
    let mut budget = PromotionStateBudget::default();
    let intents = match promotion_state_leaf_names(&promotion_root.join("intents"), &mut budget) {
        Ok(names) => names,
        Err(reason) => {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::PromotionStateInvalid,
                true,
                reason,
                None,
            ));
            return;
        }
    };
    let receipts = match promotion_state_leaf_names(&promotion_root.join("receipts"), &mut budget) {
        Ok(names) => names,
        Err(reason) => {
            result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::PromotionStateInvalid,
                true,
                reason,
                None,
            ));
            return;
        }
    };
    let all_digests = intents
        .iter()
        .chain(receipts.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut touched_isolations = BTreeSet::new();
    let mut read_bytes = 0_u64;
    let mut legacy_previews: Option<
        Result<BTreeMap<String, GovernedPromotionPreview>, PromotionApplyError>,
    > = None;
    for digest in all_digests {
        let intent = if intents.contains(&digest) {
            match read_promotion_state_document(&effect_lock, "intents", &digest, &mut read_bytes) {
                Ok(bytes) => serde_json::from_slice::<PromotionReplayIntent>(&bytes)
                    .map_err(|error| format!("promotion intent {digest} is invalid: {error}")),
                Err(error) => Err(error),
            }
            .ok()
        } else {
            None
        };
        let receipt_result = if receipts.contains(&digest) {
            match read_promotion_state_document(&effect_lock, "receipts", &digest, &mut read_bytes)
            {
                Ok(bytes) => serde_json::from_slice::<GovernedPromotionReceipt>(&bytes)
                    .map_err(|error| format!("promotion receipt {digest} is invalid: {error}")),
                Err(error) => Err(error),
            }
            .map(Some)
        } else {
            Ok(None)
        };
        let expected_digest = format!("sha256:{digest}");
        match receipt_result {
            Err(reason) => result.gaps.push(replacement_gap(
                ReplacementWorkspaceGapCode::PromotionStateInvalid,
                true,
                reason,
                None,
            )),
            Ok(Some(receipt)) => {
                let isolation_id = receipt.preview.source.isolation_id.clone();
                touched_isolations.insert(isolation_id.clone());
                let validation = (|| {
                    let intent = intent.as_ref().ok_or_else(|| {
                        PromotionApplyError::ReceiptInvalid(
                            "receipt has no matching durable intent".to_owned(),
                        )
                    })?;
                    validate_intent_request(intent, &isolation_id, &expected_digest)?;
                    verify_promotion_receipt(&receipt, &expected_digest)?;
                    verify_receipt_matches_intent(&receipt, intent)?;
                    verify_consumed_replay_binding_under_lock(binding, &effect_lock, &receipt)
                })();
                match validation {
                    Ok(()) => result.promotions.push(ReplacementPromotionInspection {
                        isolation_id,
                        status: ReplacementPromotionStatus::Completed,
                        preview_digest: Some(expected_digest),
                        receipt_digest: Some(receipt.receipt_digest),
                        summary:
                            "This promotion has a valid durable receipt and consumed replay record."
                                .to_owned(),
                    }),
                    Err(error) => {
                        result.promotions.push(ReplacementPromotionInspection {
                            isolation_id: isolation_id.clone(),
                            status: ReplacementPromotionStatus::BlockedCorrupt,
                            preview_digest: Some(expected_digest),
                            receipt_digest: Some(receipt.receipt_digest),
                            summary: format!(
                                "This promotion cannot be trusted or recovered automatically: {error}"
                            ),
                        });
                        result.gaps.push(replacement_gap(
                            ReplacementWorkspaceGapCode::PromotionStateInvalid,
                            true,
                            format!(
                                "Promotion state for isolation {} is invalid: {error}",
                                isolation_id.0
                            ),
                            Some(isolation_id),
                        ));
                    }
                }
            }
            Ok(None) => {
                let Some(intent) = intent else {
                    result.gaps.push(replacement_gap(
                        ReplacementWorkspaceGapCode::PromotionStateInvalid,
                        true,
                        format!("Promotion intent {expected_digest} could not be decoded safely."),
                        None,
                    ));
                    continue;
                };
                let inspection = inspect_split_root_effect_transaction_under_lock(
                    &binding.state_root,
                    &effect_lock,
                    PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
                    PROMOTION_EFFECT_WAL_RELATIVE_PATH,
                    &intent.transaction_id.0,
                );
                let mut preview = inspection
                    .as_ref()
                    .ok()
                    .and_then(|inspection| inspection.as_ref())
                    .and_then(|inspection| preview_from_effect_inspection(inspection).ok())
                    .or_else(|| intent.preview.clone());
                if preview.is_none()
                    && intent.schema_version == "governed_promotion_intent_v1"
                    && inspection.as_ref().is_ok_and(Option::is_none)
                {
                    if let Err(error) = validate_intent_request(
                        &intent,
                        &StableId("legacy.intent.unbound".to_owned()),
                        &expected_digest,
                    ) {
                        result.gaps.push(replacement_gap(
                            ReplacementWorkspaceGapCode::PromotionStateInvalid,
                            true,
                            format!(
                                "Promotion intent {expected_digest} is not a valid legacy recovery authority: {error}"
                            ),
                            None,
                        ));
                        continue;
                    }
                    if legacy_previews.is_none() {
                        legacy_previews = Some(reconstruct_legacy_v1_previews(
                            binding, guidance, now, selections,
                        ));
                    }
                    preview = legacy_previews
                        .as_ref()
                        .and_then(|previews| previews.as_ref().ok())
                        .and_then(|previews| previews.get(&expected_digest))
                        .cloned();
                }
                let Some(preview) = preview else {
                    let detail = legacy_previews
                        .as_ref()
                        .and_then(|previews| previews.as_ref().err())
                        .map_or_else(
                            || {
                                "no trustworthy isolation/preview binding could be reconstructed"
                                    .to_owned()
                            },
                            ToString::to_string,
                        );
                    result.gaps.push(replacement_gap(
                        ReplacementWorkspaceGapCode::PromotionStateInvalid,
                        true,
                        format!(
                            "Promotion intent {expected_digest} has no trustworthy isolation/preview binding: {detail}."
                        ),
                        None,
                    ));
                    continue;
                };
                let isolation_id = preview.source.isolation_id.clone();
                touched_isolations.insert(isolation_id.clone());
                if readiness_profile != WorkflowReadinessProfile::SoloCooperative {
                    result.promotions.push(ReplacementPromotionInspection {
                        isolation_id: isolation_id.clone(),
                        status: ReplacementPromotionStatus::BlockedCorrupt,
                        preview_digest: Some(expected_digest),
                        receipt_digest: None,
                        summary: "An incomplete local promotion exists, but this readiness profile does not admit solo recovery.".to_owned(),
                    });
                    result.gaps.push(replacement_gap(
                        ReplacementWorkspaceGapCode::PromotionRequiresSoloProfile,
                        true,
                        format!(
                            "Isolation {} has an incomplete promotion that requires the solo_cooperative profile.",
                            isolation_id.0
                        ),
                        Some(isolation_id),
                    ));
                    continue;
                }
                let validation = inspection
                    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))
                    .and_then(|inspection| {
                        validate_recoverable_promotion(
                            binding,
                            &effect_lock,
                            &intent,
                            preview,
                            inspection,
                            &expected_digest,
                        )
                    });
                match validation {
                    Ok(()) => result.promotions.push(ReplacementPromotionInspection {
                        isolation_id,
                        status: ReplacementPromotionStatus::Recoverable,
                        preview_digest: Some(expected_digest),
                        receipt_digest: None,
                        summary: "A previous promotion stopped safely. Recover it before starting new work.".to_owned(),
                    }),
                    Err(error) => {
                        result.promotions.push(ReplacementPromotionInspection {
                            isolation_id: isolation_id.clone(),
                            status: ReplacementPromotionStatus::BlockedCorrupt,
                            preview_digest: Some(expected_digest),
                            receipt_digest: None,
                            summary: format!(
                                "The incomplete promotion is not safe to recover automatically: {error}"
                            ),
                        });
                        result.gaps.push(replacement_gap(
                            ReplacementWorkspaceGapCode::PromotionStateInvalid,
                            true,
                            format!(
                                "Promotion state for isolation {} needs manual repair: {error}",
                                isolation_id.0
                            ),
                            Some(isolation_id),
                        ));
                    }
                }
            }
        }
    }
    if let Err(error) = effect_lock.validate_retained_lock_file() {
        result.gaps.push(replacement_gap(
            ReplacementWorkspaceGapCode::PromotionStateInvalid,
            true,
            format!("Promotion state changed during read-only inspection: {error}"),
            None,
        ));
    }
    add_not_started_promotions(selections, &touched_isolations, result);
}

fn reconstruct_legacy_v1_previews(
    binding: &WorkflowGovernanceProjectBinding,
    guidance: &WorkflowGovernanceGuidance,
    now: u64,
    selections: &[IsolationSelection],
) -> Result<BTreeMap<String, GovernedPromotionPreview>, PromotionApplyError> {
    let destination = RetainedProjectTree::capture(
        &binding.project_root,
        MAX_PROMOTION_SNAPSHOT_ENTRIES,
        MAX_PROMOTION_SNAPSHOT_BYTES,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let claim_wal = binding.state_root.join(CLAIM_WAL_RELATIVE_PATH);
    let claim_lock = binding.state_root.join(CLAIM_WAL_LOCK_RELATIVE_PATH);
    let claim_projection = match (
        path_exists_no_follow(&claim_wal).map_err(PromotionApplyError::Preview)?,
        path_exists_no_follow(&claim_lock).map_err(PromotionApplyError::Preview)?,
    ) {
        (false, false) => None,
        (true, true) => Some(
            project_existing_claim_wal(
                &binding.state_root,
                &ClaimWalProjectionOptions {
                    repair: false,
                    stop_policy: ClaimWalProjectionStopPolicy::RequireCleanEof,
                },
            )
            .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?,
        ),
        _ => {
            return Err(PromotionApplyError::RecoveryRequired(
                "claim WAL and its existing lock must both be present or both be absent".to_owned(),
            ))
        }
    };
    let mut previews = BTreeMap::new();
    for isolation in selections
        .iter()
        .filter(|selection| selection.contract.status == IsolationStatus::Active)
    {
        let Ok(prepared) = derive_governed_promotion(
            binding,
            &isolation.contract.id,
            guidance,
            &destination,
            now,
            claim_projection.as_ref(),
        ) else {
            continue;
        };
        previews.insert(
            prepared.preview.preview_digest.clone(),
            prepared.preview.clone(),
        );
    }
    destination
        .revalidate()
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    Ok(previews)
}

fn add_not_started_promotions(
    selections: &[IsolationSelection],
    touched: &BTreeSet<StableId>,
    result: &mut ReplacementWorkspaceInspection,
) {
    result.promotions.extend(
        selections
            .iter()
            .filter(|isolation| !touched.contains(&isolation.contract.id))
            .map(|isolation| ReplacementPromotionInspection {
                isolation_id: isolation.contract.id.clone(),
                status: ReplacementPromotionStatus::NotStarted,
                preview_digest: None,
                receipt_digest: None,
                summary: "No durable promotion has started for this isolation.".to_owned(),
            }),
    );
}

fn promotion_state_leaf_names(
    directory: &Path,
    budget: &mut PromotionStateBudget,
) -> Result<BTreeSet<String>, String> {
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => {
            return Err(format!(
                "Promotion state directory {} could not be inspected: {error}",
                directory.display()
            ))
        }
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "Promotion state directory {} is not a no-follow directory.",
                directory.display()
            ))
        }
        Ok(_) => {}
    }
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Could not list {}: {error}", directory.display()))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        if budget.documents >= MAX_PROMOTION_STATE_DOCUMENTS {
            return Err(format!(
                "Promotion state exceeds the combined limit of {MAX_PROMOTION_STATE_DOCUMENTS} intent and receipt documents."
            ));
        }
        let entry = entry.map_err(|error| {
            format!(
                "Could not inspect an entry in {}: {error}",
                directory.display()
            )
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| {
            format!(
                "Could not inspect promotion state entry {}: {error}",
                entry.path().display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "Promotion state entry {} is not a no-follow regular file.",
                entry.path().display()
            ));
        }
        budget.documents = budget.documents.saturating_add(1);
        budget.bytes = budget.bytes.saturating_add(metadata.len());
        if budget.bytes > MAX_PROMOTION_STATE_TOTAL_BYTES {
            return Err(format!(
                "Promotion state exceeds the combined byte budget of {MAX_PROMOTION_STATE_TOTAL_BYTES} bytes."
            ));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Promotion state contains a non-UTF-8 leaf name.".to_owned())?;
        let Some(hex) = name.strip_suffix(".json") else {
            return Err(format!(
                "Promotion state leaf {name} has an unsupported name."
            ));
        };
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "Promotion state leaf {name} is not a lowercase sha256 name."
            ));
        }
        names.insert(hex.to_owned());
    }
    Ok(names)
}

fn read_promotion_state_document(
    effect_lock: &EffectStoreLock,
    directory: &str,
    digest_hex: &str,
    total_read_bytes: &mut u64,
) -> Result<Vec<u8>, String> {
    if *total_read_bytes >= MAX_PROMOTION_STATE_TOTAL_BYTES {
        return Err(format!(
            "Promotion state exceeds the combined read budget of {MAX_PROMOTION_STATE_TOTAL_BYTES} bytes."
        ));
    }
    let remaining_budget = MAX_PROMOTION_STATE_TOTAL_BYTES - *total_read_bytes;
    let io = effect_lock
        .retained_store_io()
        .map_err(|error| error.to_string())?;
    let directory = io
        .retain_existing_subdirectory(Path::new(directory))
        .map_err(|error| error.to_string())?;
    let leaf = format!("{digest_hex}.json");
    let mut witness = directory
        .read_optional_bounded(
            Path::new(&leaf),
            PROMOTION_RECEIPT_MAX_BYTES.min(remaining_budget),
        )
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Promotion state leaf {leaf} disappeared during inspection."))?;
    let bytes = witness.raw_bytes().to_vec();
    *total_read_bytes =
        (*total_read_bytes).saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    if *total_read_bytes > MAX_PROMOTION_STATE_TOTAL_BYTES {
        return Err(format!(
            "Promotion state exceeds the combined read budget of {MAX_PROMOTION_STATE_TOTAL_BYTES} bytes."
        ));
    }
    witness.revalidate().map_err(|error| error.to_string())?;
    directory.validate().map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn verify_receipt_matches_intent(
    receipt: &GovernedPromotionReceipt,
    intent: &PromotionReplayIntent,
) -> Result<(), PromotionApplyError> {
    let original_provenance = receipt
        .recovery_execution
        .as_ref()
        .map_or(&receipt.provenance_digest, |recovery| {
            &recovery.superseded_provenance_digest
        });
    let original_publication = receipt
        .recovery_execution
        .as_ref()
        .map_or(&receipt.publication_capability_digest, |recovery| {
            &recovery.superseded_publication_capability_digest
        });
    if receipt.derived_principal_id != intent.principal_id
        || receipt.transaction_id != intent.transaction_id
        || receipt.effect_id != intent.effect_id
        || receipt.replay != intent.replay
        || original_provenance != &intent.provenance_digest
        || original_publication != &intent.publication_capability_digest
        || intent
            .preview
            .as_ref()
            .is_some_and(|preview| preview != &receipt.preview)
    {
        return Err(PromotionApplyError::ReceiptInvalid(
            "receipt differs from its matching durable intent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_recoverable_promotion(
    binding: &WorkflowGovernanceProjectBinding,
    effect_lock: &EffectStoreLock,
    intent: &PromotionReplayIntent,
    preview: GovernedPromotionPreview,
    inspection: Option<SplitRootEffectTransactionInspection>,
    expected_preview_digest: &str,
) -> Result<(), PromotionApplyError> {
    let isolation_id = preview.source.isolation_id.clone();
    validate_intent_request(intent, &isolation_id, expected_preview_digest)?;
    let destination_tree = RetainedProjectTree::capture(
        &binding.project_root,
        MAX_PROMOTION_SNAPSHOT_ENTRIES,
        MAX_PROMOTION_SNAPSHOT_BYTES,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let prepared = prepare_recovery_from_stored_preview(
        binding,
        &isolation_id,
        expected_preview_digest,
        preview,
        &destination_tree,
    )?;
    validate_recovery_preview_identity(&prepared.preview, expected_preview_digest)?;
    if inspection.is_none() {
        validate_pre_begin_destination_exact(binding, &destination_tree, &prepared.preview)?;
    }
    validate_recovery_destination(&destination_tree, &prepared.source_tree, &prepared.preview)?;
    let (effect, _payloads, _applied_files) = promotion_effect_and_payloads(&prepared)?;
    let original_provenance = promotion_provenance_from_intent(intent, &prepared.preview, &effect)?;
    let legacy_v1_pre_begin =
        intent.schema_version == "governed_promotion_intent_v1" && inspection.is_none();
    if !legacy_v1_pre_begin && original_provenance.digest != intent.provenance_digest {
        return Err(PromotionApplyError::RecoveryRequired(
            "durable intent provenance differs from its preview/effect binding".to_owned(),
        ));
    }
    let replay_binding = EffectReplayCommitBinding::new(
        intent.replay.key_hash.clone(),
        intent.replay.intent_digest.clone(),
        intent.replay.commit_digest.clone(),
        intent.replay.reservation_revision,
    );
    if let Some(inspection) = &inspection {
        if inspection.effect_id != effect.tool_effect_contract.id
            || inspection.replay_binding != replay_binding
        {
            return Err(PromotionApplyError::RecoveryRequired(
                "effect WAL begin contradicts the durable promotion intent".to_owned(),
            ));
        }
        if inspection.provenance != original_provenance {
            let publication_capability_digest =
                recovery_publication_capability_from_wal(&inspection.provenance)?;
            let recovery_execution = recovery_execution_binding(intent);
            let expected_recovery = pre_begin_recovery_provenance(
                intent,
                &prepared.preview,
                &effect,
                &publication_capability_digest,
                &recovery_execution,
            )?;
            if inspection.provenance != expected_recovery {
                return Err(PromotionApplyError::RecoveryRequired(
                    "effect WAL provenance does not link to the durable original intent".to_owned(),
                ));
            }
        }
        if inspection.stage == SplitRootEffectTransactionStage::RolledBack {
            return Err(PromotionApplyError::RecoveryRequired(
                "the interrupted promotion was rolled back and cannot be resumed".to_owned(),
            ));
        }
    }
    let replay = inspect_replay_key_hash_under_effect_lock(
        &binding.state_root,
        effect_lock,
        PROMOTION_EFFECT_LOCK_RELATIVE_PATH,
        &intent.replay.key_hash,
        &intent.replay.intent_digest,
        &intent.replay.commit_digest,
    )
    .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))?;
    let exact_reserved = replay.as_ref().is_some_and(|reservation| {
        reservation.state == ReplayReservationState::Reserved
            && reservation.revision == intent.replay.reservation_revision
            && reservation.consumed_seq.is_none()
    });
    let exact_consumed = replay.as_ref().is_some_and(|reservation| {
        reservation.state == ReplayReservationState::Consumed
            && reservation.revision == intent.replay.reservation_revision.saturating_add(1)
            && reservation.consumed_seq.is_some()
    });
    let replay_is_safe = match inspection.as_ref().map(|inspection| inspection.stage) {
        None => replay.is_none() || exact_reserved,
        Some(SplitRootEffectTransactionStage::Begun) => exact_reserved,
        Some(SplitRootEffectTransactionStage::Committed) => exact_reserved || exact_consumed,
        Some(SplitRootEffectTransactionStage::ReplayConsumed) => exact_consumed,
        Some(SplitRootEffectTransactionStage::RolledBack) => false,
        Some(_) => false,
    };
    if !replay_is_safe {
        return Err(PromotionApplyError::RecoveryRequired(
            "replay state is missing or contradicts the interrupted promotion stage".to_owned(),
        ));
    }
    prepared.source_tree.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "isolation source changed during inspection: {error}"
        ))
    })?;
    destination_tree.revalidate().map_err(|error| {
        PromotionApplyError::RecoveryRequired(format!(
            "canonical project changed during inspection: {error}"
        ))
    })?;
    effect_lock
        .validate_retained_lock_file()
        .map_err(|error| PromotionApplyError::RecoveryRequired(error.to_string()))
}

fn load_active_isolation(
    state_root: &Path,
    isolation_id: &StableId,
) -> Result<IsolationSelection, PromotionPreviewError> {
    let mut selected = load_isolation_registry(state_root)?
        .into_iter()
        .filter(|isolation| isolation.contract.id == *isolation_id)
        .collect::<Vec<_>>();
    let isolation = match selected.len() {
        0 => {
            return Err(PromotionPreviewError::IsolationNotFound(
                isolation_id.0.clone(),
            ))
        }
        1 => selected.pop().expect("one selected isolation"),
        _ => {
            return Err(PromotionPreviewError::DuplicateIsolationId(
                isolation_id.0.clone(),
            ))
        }
    };
    if isolation.contract.status != IsolationStatus::Active {
        return Err(PromotionPreviewError::IsolationNotActive(
            isolation_id.0.clone(),
        ));
    }
    Ok(isolation)
}

fn load_isolation_registry(
    state_root: &Path,
) -> Result<Vec<IsolationSelection>, PromotionPreviewError> {
    let isolation_dir = state_root.join("contracts").join("isolations");
    let isolation_directory_metadata = fs::symlink_metadata(&isolation_dir).map_err(|error| {
        PromotionPreviewError::IsolationDirectory {
            path: isolation_dir.clone(),
            source: error.to_string(),
        }
    })?;
    if isolation_directory_metadata.file_type().is_symlink()
        || !isolation_directory_metadata.is_dir()
    {
        return Err(PromotionPreviewError::IsolationDirectory {
            path: isolation_dir,
            source: "registry root must be a no-follow directory".to_owned(),
        });
    }
    let entries = fs::read_dir(&isolation_dir).map_err(|error| {
        PromotionPreviewError::IsolationDirectory {
            path: isolation_dir.clone(),
            source: error.to_string(),
        }
    })?;
    let mut paths = entries
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                PromotionPreviewError::IsolationDirectory {
                    path: isolation_dir.clone(),
                    source: error.to_string(),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    if paths.len() > MAX_ISOLATION_DOCUMENTS {
        return Err(PromotionPreviewError::TooManyIsolationDocuments);
    }
    let mut selected = Vec::new();
    let mut total_bytes = 0_u64;
    for path in paths {
        if path.extension().and_then(|extension| extension.to_str()) != Some("yaml") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            PromotionPreviewError::IsolationDocument {
                path: path.clone(),
                source: error.to_string(),
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(PromotionPreviewError::IsolationDocument {
                path,
                source: "registry entries must be no-follow regular files".to_owned(),
            });
        }
        let raw = read_bounded(&path, MAX_ISOLATION_DOCUMENT_BYTES)?;
        total_bytes = total_bytes.saturating_add(u64::try_from(raw.len()).unwrap_or(u64::MAX));
        if total_bytes > MAX_ISOLATION_DOCUMENT_TOTAL_BYTES {
            return Err(PromotionPreviewError::IsolationDirectory {
                path: isolation_dir.clone(),
                source: format!(
                    "registry documents exceed the combined byte budget of {MAX_ISOLATION_DOCUMENT_TOTAL_BYTES} bytes"
                ),
            });
        }
        let document: IsolationContractDocument =
            yaml_serde::from_slice(&raw).map_err(|error| {
                PromotionPreviewError::IsolationDocument {
                    path: path.clone(),
                    source: error.to_string(),
                }
            })?;
        if document.schema_version.trim().is_empty() {
            return Err(PromotionPreviewError::IsolationDocument {
                path,
                source: "schema_version must not be blank".to_owned(),
            });
        }
        validate_isolation_contract(&document.isolation_contract)
            .map_err(|error| PromotionPreviewError::IsolationInvalid(error.to_string()))?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| PromotionPreviewError::NonUtf8Path(path.clone()))?;
        selected.push(IsolationSelection {
            relative_path: format!("contracts/isolations/{file_name}"),
            raw_digest: sha256_content_hash(&raw),
            contract: document.isolation_contract,
        });
    }
    Ok(selected)
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, PromotionPreviewError> {
    let mut file = open_git_file_no_follow(path).map_err(|error| {
        PromotionPreviewError::IsolationDocument {
            path: path.to_path_buf(),
            source: error.to_string(),
        }
    })?;
    let opened_metadata =
        file.metadata()
            .map_err(|error| PromotionPreviewError::IsolationDocument {
                path: path.to_path_buf(),
                source: error.to_string(),
            })?;
    if !opened_metadata.is_file() || !metadata_has_single_link(&opened_metadata) {
        return Err(PromotionPreviewError::IsolationDocument {
            path: path.to_path_buf(),
            source: "opened document must be a single-link no-follow regular file".to_owned(),
        });
    }
    let mut raw = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut raw)
        .map_err(|error| PromotionPreviewError::IsolationDocument {
            path: path.to_path_buf(),
            source: error.to_string(),
        })?;
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > maximum {
        return Err(PromotionPreviewError::IsolationDocument {
            path: path.to_path_buf(),
            source: format!("document exceeds {maximum} bytes"),
        });
    }
    let reopened = open_git_file_no_follow(path).map_err(|error| {
        PromotionPreviewError::IsolationDocument {
            path: path.to_path_buf(),
            source: format!("document changed during retained read: {error}"),
        }
    })?;
    let reopened_metadata =
        reopened
            .metadata()
            .map_err(|error| PromotionPreviewError::IsolationDocument {
                path: path.to_path_buf(),
                source: format!("document changed during retained read: {error}"),
            })?;
    if !same_file_identity(&opened_metadata, &reopened_metadata) {
        return Err(PromotionPreviewError::IsolationDocument {
            path: path.to_path_buf(),
            source: "document identity changed during retained read".to_owned(),
        });
    }
    Ok(raw)
}

#[cfg(unix)]
fn metadata_has_single_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() == 1
}

#[cfg(windows)]
fn metadata_has_single_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata.number_of_links() == Some(1)
}

#[cfg(not(any(unix, windows)))]
fn metadata_has_single_link(_metadata: &std::fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
}

#[cfg(not(any(unix, windows)))]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn canonical_directory(path: &Path, field: &'static str) -> Result<PathBuf, PromotionPreviewError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| PromotionPreviewError::SourceRoot {
            path: path.to_path_buf(),
            source: format!("{field}: {error}"),
        })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PromotionPreviewError::SourceRoot {
            path: path.to_path_buf(),
            source: format!("{field} must be a no-follow directory"),
        });
    }
    fs::canonicalize(path).map_err(|error| PromotionPreviewError::SourceRoot {
        path: path.to_path_buf(),
        source: format!("{field}: {error}"),
    })
}

fn path_exists_no_follow(path: &Path) -> Result<bool, PromotionPreviewError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(PromotionPreviewError::ClaimProjection(format!(
                    "{} is not a no-follow regular file",
                    path.display()
                )));
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(PromotionPreviewError::ClaimProjection(format!(
            "{} is unavailable: {error}",
            path.display()
        ))),
    }
}

fn inventory_files(tree: &RetainedProjectTree) -> Vec<PromotionInventoryFile> {
    tree.regular_file_observations()
        .into_iter()
        .map(|file| PromotionInventoryFile {
            relative_path: file.relative_path,
            content_digest: file.content_digest,
            byte_length: file.byte_length,
            metadata_fingerprint: file.metadata_fingerprint,
        })
        .collect()
}

fn inventory_directories(tree: &RetainedProjectTree) -> Vec<PromotionInventoryDirectory> {
    tree.directory_observations()
        .into_iter()
        .map(|directory| PromotionInventoryDirectory {
            relative_path: directory.relative_path,
            metadata_fingerprint: directory.metadata_fingerprint,
        })
        .collect()
}

fn snapshot_binding(
    canonical_root: &Path,
    tree: &RetainedProjectTree,
    excluded_roots: Vec<PromotionExcludedRootBinding>,
) -> Result<PromotionSnapshotBinding, PromotionPreviewError> {
    let root = canonical_root
        .to_str()
        .ok_or_else(|| PromotionPreviewError::NonUtf8Path(canonical_root.to_path_buf()))?
        .to_owned();
    let files = inventory_files(tree);
    let directories = inventory_directories(tree);
    let snapshot_digest = promotion_domain_digest(
        "promotion.filesystem_snapshot.v1",
        &(files.as_slice(), directories.as_slice()),
    )?;
    let total_regular_file_bytes = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.byte_length)
            .ok_or(PromotionPreviewError::SnapshotBindingMismatch)
    })?;
    Ok(PromotionSnapshotBinding {
        canonical_root_digest: promotion_domain_digest("promotion.canonical_root.v1", &root)?,
        canonical_root: root,
        snapshot_digest,
        retained_tree_digest: tree.snapshot_digest().to_owned(),
        regular_file_set_digest: tree.regular_file_snapshot_digest().to_owned(),
        file_count: files.len(),
        directory_count: directories.len(),
        total_regular_file_bytes,
        excluded_roots,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitWorktreeObservation {
    binding: PromotionGitWorktreeBinding,
}

fn declared_worktree_candidate(
    project_root: &Path,
    declared: &RepoPath,
) -> Result<PathBuf, PromotionPreviewError> {
    let path = Path::new(&declared.0);
    if path.is_absolute() {
        return Err(PromotionPreviewError::GitWorktree(
            "worktree_path must be project-relative".to_owned(),
        ));
    }
    let components = path.components().collect::<Vec<_>>();
    if components.first() != Some(&std::path::Component::ParentDir)
        || components.len() < 3
        || components
            .iter()
            .skip(1)
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(PromotionPreviewError::GitWorktree(
            "worktree_path must contain exactly one leading '..' followed by normal components"
                .to_owned(),
        ));
    }
    Ok(project_root.join(path))
}

fn observe_git_worktree(
    source_root: &Path,
    destination_root: &Path,
    contract_branch: &str,
) -> Result<GitWorktreeObservation, PromotionPreviewError> {
    let destination_git =
        canonical_no_follow_directory(&destination_root.join(".git"), "canonical .git")?;
    let source_dot_git = source_root.join(".git");
    let source_git_pointer = read_git_regular_file(&source_dot_git, 16 * 1024)?;
    let pointer = std::str::from_utf8(&source_git_pointer)
        .map_err(|_| {
            PromotionPreviewError::GitWorktree("worktree .git pointer is not UTF-8".to_owned())
        })?
        .trim();
    let gitdir_value = pointer.strip_prefix("gitdir: ").ok_or_else(|| {
        PromotionPreviewError::GitWorktree(
            "source .git must be a linked-worktree gitdir pointer, not an ordinary directory"
                .to_owned(),
        )
    })?;
    let gitdir_candidate = PathBuf::from(gitdir_value);
    let gitdir_candidate = if gitdir_candidate.is_absolute() {
        gitdir_candidate
    } else {
        source_root.join(gitdir_candidate)
    };
    let worktree_git_dir = canonical_no_follow_directory(&gitdir_candidate, "worktree gitdir")?;
    let worktrees_root = destination_git.join("worktrees");
    let relative_worktree_git_dir =
        worktree_git_dir
            .strip_prefix(&worktrees_root)
            .map_err(|_| {
                PromotionPreviewError::GitWorktree(
            "source gitdir is not registered under the canonical repository worktrees namespace"
                .to_owned(),
        )
            })?;
    if relative_worktree_git_dir.components().count() != 1
        || !relative_worktree_git_dir
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(PromotionPreviewError::GitWorktree(
            "source gitdir is not one exact canonical worktree registration".to_owned(),
        ));
    }
    let registered_dot_git_raw =
        read_git_regular_file(&worktree_git_dir.join("gitdir"), 16 * 1024)?;
    let registered_dot_git_text = std::str::from_utf8(&registered_dot_git_raw)
        .map_err(|_| {
            PromotionPreviewError::GitWorktree(
                "registered worktree gitdir pointer is not UTF-8".to_owned(),
            )
        })?
        .trim();
    let registered_dot_git_candidate = PathBuf::from(registered_dot_git_text);
    let registered_dot_git_candidate = if registered_dot_git_candidate.is_absolute() {
        registered_dot_git_candidate
    } else {
        worktree_git_dir.join(registered_dot_git_candidate)
    };
    let registered_dot_git = canonical_no_follow_regular_file(
        &registered_dot_git_candidate,
        "registered worktree .git",
    )?;
    let source_dot_git = canonical_no_follow_regular_file(&source_dot_git, "source worktree .git")?;
    if registered_dot_git != source_dot_git {
        return Err(PromotionPreviewError::GitWorktree(
            "source path is not the worktree registered by the canonical repository".to_owned(),
        ));
    }
    let commondir_raw = read_git_regular_file(&worktree_git_dir.join("commondir"), 4096)?;
    let commondir_text = std::str::from_utf8(&commondir_raw)
        .map_err(|_| PromotionPreviewError::GitWorktree("commondir is not UTF-8".to_owned()))?
        .trim();
    let common_git_dir = canonical_no_follow_directory(
        &worktree_git_dir.join(commondir_text),
        "worktree common repository",
    )?;
    if common_git_dir != destination_git {
        return Err(PromotionPreviewError::GitWorktree(
            "source worktree belongs to a different Git repository".to_owned(),
        ));
    }
    let source_head = read_git_regular_file(&worktree_git_dir.join("HEAD"), 4096)?;
    let source_head_text = std::str::from_utf8(&source_head)
        .map_err(|_| PromotionPreviewError::GitWorktree("source HEAD is not UTF-8".to_owned()))?
        .trim();
    let expected_branch_ref = format!("refs/heads/{contract_branch}");
    let source_branch_ref = source_head_text.strip_prefix("ref: ").ok_or_else(|| {
        PromotionPreviewError::GitWorktree("source worktree HEAD must not be detached".to_owned())
    })?;
    if source_branch_ref != expected_branch_ref {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "source branch {source_branch_ref} differs from isolation contract {expected_branch_ref}"
        )));
    }
    let head_oid = resolve_git_ref(&common_git_dir, source_branch_ref)?;
    let canonical_head = read_git_regular_file(&common_git_dir.join("HEAD"), 4096)?;
    let canonical_head_text = std::str::from_utf8(&canonical_head)
        .map_err(|_| PromotionPreviewError::GitWorktree("canonical HEAD is not UTF-8".to_owned()))?
        .trim();
    let (canonical_repository_head_ref, canonical_repository_head_oid) =
        if let Some(reference) = canonical_head_text.strip_prefix("ref: ") {
            (
                reference.to_owned(),
                resolve_git_ref(&common_git_dir, reference)?,
            )
        } else {
            validate_git_oid(canonical_head_text)?;
            ("detached".to_owned(), canonical_head_text.to_owned())
        };
    let common_repository_git_dir_digest = promotion_domain_digest(
        "promotion.git.common_repository_path.v1",
        &common_git_dir.to_string_lossy(),
    )?;
    let worktree_git_dir_digest = promotion_domain_digest(
        "promotion.git.worktree_gitdir_path.v1",
        &worktree_git_dir.to_string_lossy(),
    )?;
    let observation_digest = promotion_domain_digest(
        "promotion.git.worktree_observation.v1",
        &(
            &common_repository_git_dir_digest,
            &worktree_git_dir_digest,
            source_branch_ref,
            &head_oid,
            &canonical_repository_head_ref,
            &canonical_repository_head_oid,
            sha256_content_hash(&source_git_pointer),
            sha256_content_hash(&registered_dot_git_raw),
            sha256_content_hash(&commondir_raw),
            sha256_content_hash(&source_head),
            sha256_content_hash(&canonical_head),
        ),
    )?;
    Ok(GitWorktreeObservation {
        binding: PromotionGitWorktreeBinding {
            common_repository_git_dir_digest,
            worktree_git_dir_digest,
            branch_ref: source_branch_ref.to_owned(),
            head_oid,
            canonical_repository_head_ref,
            canonical_repository_head_oid,
            observation_digest,
        },
    })
}

fn canonical_no_follow_directory(
    path: &Path,
    label: &str,
) -> Result<PathBuf, PromotionPreviewError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PromotionPreviewError::GitWorktree(format!("{label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "{label} must be a no-follow directory"
        )));
    }
    fs::canonicalize(path)
        .map_err(|error| PromotionPreviewError::GitWorktree(format!("{label}: {error}")))
}

fn canonical_no_follow_regular_file(
    path: &Path,
    label: &str,
) -> Result<PathBuf, PromotionPreviewError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PromotionPreviewError::GitWorktree(format!("{label}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "{label} must be a no-follow regular file"
        )));
    }
    fs::canonicalize(path)
        .map_err(|error| PromotionPreviewError::GitWorktree(format!("{label}: {error}")))
}

fn read_git_regular_file(path: &Path, maximum: u64) -> Result<Vec<u8>, PromotionPreviewError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        PromotionPreviewError::GitWorktree(format!("{}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "{} must be a no-follow regular file",
            path.display()
        )));
    }
    let mut file = open_git_file_no_follow(path).map_err(|error| {
        PromotionPreviewError::GitWorktree(format!("{}: {error}", path.display()))
    })?;
    let opened_metadata = file.metadata().map_err(|error| {
        PromotionPreviewError::GitWorktree(format!("{}: {error}", path.display()))
    })?;
    if !opened_metadata.is_file() {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "{} opened object is not a regular file",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            PromotionPreviewError::GitWorktree(format!("{}: {error}", path.display()))
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "{} exceeds {maximum} bytes",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_git_file_no_follow(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(windows)]
fn open_git_file_no_follow(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_git_file_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

fn resolve_git_ref(
    common_git_dir: &Path,
    reference: &str,
) -> Result<String, PromotionPreviewError> {
    let ref_path = Path::new(reference);
    if ref_path.is_absolute()
        || ref_path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(PromotionPreviewError::GitWorktree(format!(
            "invalid Git ref {reference}"
        )));
    }
    let loose_path = common_git_dir.join(ref_path);
    match fs::symlink_metadata(&loose_path) {
        Ok(_) => {
            let bytes = read_git_regular_file(&loose_path, 4096)?;
            let oid = std::str::from_utf8(&bytes)
                .map_err(|_| {
                    PromotionPreviewError::GitWorktree("loose ref is not UTF-8".to_owned())
                })?
                .trim();
            validate_git_oid(oid)?;
            Ok(oid.to_owned())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let packed =
                read_git_regular_file(&common_git_dir.join("packed-refs"), 8 * 1024 * 1024)?;
            let text = std::str::from_utf8(&packed).map_err(|_| {
                PromotionPreviewError::GitWorktree("packed-refs is not UTF-8".to_owned())
            })?;
            for line in text.lines() {
                if line.starts_with('#') || line.starts_with('^') || line.trim().is_empty() {
                    continue;
                }
                if let Some((oid, candidate)) = line.split_once(' ') {
                    if candidate == reference {
                        validate_git_oid(oid)?;
                        return Ok(oid.to_owned());
                    }
                }
            }
            Err(PromotionPreviewError::GitWorktree(format!(
                "Git ref {reference} is unresolved"
            )))
        }
        Err(error) => Err(PromotionPreviewError::GitWorktree(format!(
            "{}: {error}",
            loose_path.display()
        ))),
    }
}

fn validate_git_oid(oid: &str) -> Result<(), PromotionPreviewError> {
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PromotionPreviewError::GitWorktree(
            "Git object id must be a 40- or 64-digit hexadecimal value".to_owned(),
        ));
    }
    Ok(())
}

fn excluded_root_bindings(
    canonical_root: &Path,
) -> Result<Vec<PromotionExcludedRootBinding>, PromotionPreviewError> {
    EXCLUDED_ROOT_NAMES
        .iter()
        .map(|name| {
            let path = canonical_root.join(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    let object_kind = if metadata.file_type().is_symlink() {
                        "symlink"
                    } else if metadata.is_dir() {
                        "directory"
                    } else if metadata.is_file() {
                        "file"
                    } else {
                        "special"
                    };
                    let metadata_digest = promotion_domain_digest(
                        "promotion.excluded_root_metadata.v1",
                        &(
                            *name,
                            object_kind,
                            metadata.len(),
                            metadata.permissions().readonly(),
                        ),
                    )?;
                    Ok(PromotionExcludedRootBinding {
                        name: (*name).to_owned(),
                        kind: excluded_root_kind(name),
                        present: true,
                        metadata_digest: Some(metadata_digest),
                        promotable_content_observed: false,
                    })
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(PromotionExcludedRootBinding {
                        name: (*name).to_owned(),
                        kind: excluded_root_kind(name),
                        present: false,
                        metadata_digest: None,
                        promotable_content_observed: false,
                    })
                }
                Err(error) => Err(PromotionPreviewError::SourceRoot {
                    path,
                    source: error.to_string(),
                }),
            }
        })
        .collect()
}

fn excluded_root_kind(name: &str) -> PromotionExcludedRootKind {
    match name {
        ".git" => PromotionExcludedRootKind::GitControlMetadata,
        ".forge-method" => PromotionExcludedRootKind::ForgeControlState,
        _ => PromotionExcludedRootKind::BuildOrDependencyCache,
    }
}

const fn assurance_claim_status(
    status: WorkflowClaimResultStatus,
) -> PromotionAssuranceClaimStatus {
    match status {
        WorkflowClaimResultStatus::Unknown => PromotionAssuranceClaimStatus::Unknown,
        WorkflowClaimResultStatus::Supported => PromotionAssuranceClaimStatus::Supported,
        WorkflowClaimResultStatus::Verified => PromotionAssuranceClaimStatus::Verified,
        WorkflowClaimResultStatus::Waived => PromotionAssuranceClaimStatus::Waived,
        WorkflowClaimResultStatus::Disproven => PromotionAssuranceClaimStatus::Disproven,
        WorkflowClaimResultStatus::Contradictory => PromotionAssuranceClaimStatus::Contradictory,
    }
}
