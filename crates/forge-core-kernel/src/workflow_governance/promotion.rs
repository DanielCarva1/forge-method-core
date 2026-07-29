//! Read-only governed promotion preview.
//!
//! This module binds one Active isolation contract to retained source and
//! destination trees plus current objective, evidence, ledger, and claim-WAL
//! projections. It deliberately has no apply, WAL append, or replay surface.

use super::adapter::{WorkflowGovernanceGuidance, WorkflowGovernanceProjectBinding};
use forge_core_contracts::isolation::{
    IsolationContract, IsolationContractDocument, IsolationStatus,
};
use forge_core_contracts::{
    ClaimId, GovernedPromotionPreview, GovernedPromotionPreviewAuthority,
    PromotionAssuranceClaimCoverage, PromotionAssuranceClaimStatus, PromotionClaimConflict,
    PromotionClaimSetBinding, PromotionDestinationBinding, PromotionEvidenceRecordBinding,
    PromotionEvidenceSetBinding, PromotionExcludedRootBinding, PromotionExcludedRootKind,
    PromotionGitWorktreeBinding, PromotionGovernanceBinding, PromotionObjectiveBinding,
    PromotionObjectiveCoverage, PromotionObjectiveCoverageStatus, PromotionPathClaimAttribution,
    PromotionSnapshotBinding, PromotionSourceBinding, PromotionUnsupportedEffect,
    PromotionUnsupportedEffectKind, PromotionWriteClaimCoverage, PromotionWriteClaimCoverageStatus,
    RepoPath, StableId, WorkflowCooperativeEvidenceCurrentStatus,
    WorkflowCooperativeEvidenceDisposition, WorkflowReadinessProfile,
    GOVERNED_PROMOTION_PREVIEW_SCHEMA_VERSION,
};
use forge_core_decisions::{
    check_write_against_claims, derive_promotion_diff, evaluate_promotion_readiness, is_live,
    promotion_domain_digest, rfc3339_to_unix, validate_isolation_contract,
    PromotionInventoryDirectory, PromotionInventoryFile, PromotionPlanningError,
    PromotionReadinessInput, WorkflowClaimResultStatus, WriteCheck,
};
use forge_core_store::claim_wal::{
    project_existing_claim_wal, ClaimWalProjectionOptions, ClaimWalProjectionStopPolicy,
    CLAIM_WAL_LOCK_RELATIVE_PATH, CLAIM_WAL_RELATIVE_PATH,
};
use forge_core_store::retained_project_tree::{RetainedProjectTree, RetainedProjectTreeError};
use forge_core_store::sha256_content_hash;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};

const MAX_PROMOTION_SNAPSHOT_ENTRIES: usize = 200_000;
const MAX_PROMOTION_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ISOLATION_DOCUMENTS: usize = 10_000;
const MAX_ISOLATION_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const EXCLUDED_ROOT_NAMES: &[&str] = &[".git", ".forge-method", "target", "node_modules"];

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

pub(super) fn preview_governed_promotion(
    binding: &WorkflowGovernanceProjectBinding,
    isolation_id: &StableId,
    guidance: &WorkflowGovernanceGuidance,
    destination_tree: &RetainedProjectTree,
    now: u64,
) -> Result<GovernedPromotionPreview, PromotionPreviewError> {
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
    let claim_projection = claim_lock_present
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
        .transpose()?;
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
    let unsatisfied_source_claim_refs = assurance_claim_coverage
        .iter()
        .filter(|claim| {
            !matches!(
                claim.status,
                PromotionAssuranceClaimStatus::Verified | PromotionAssuranceClaimStatus::Waived
            )
        })
        .map(|claim| claim.claim_ref.0.clone())
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
        unsatisfied_source_claim_refs: &unsatisfied_source_claim_refs,
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
    if claim_projection.is_none() {
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
    Ok(preview)
}

fn load_active_isolation(
    state_root: &Path,
    isolation_id: &StableId,
) -> Result<IsolationSelection, PromotionPreviewError> {
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
        let document: IsolationContractDocument =
            yaml_serde::from_slice(&raw).map_err(|error| {
                PromotionPreviewError::IsolationDocument {
                    path: path.clone(),
                    source: error.to_string(),
                }
            })?;
        validate_isolation_contract(&document.isolation_contract)
            .map_err(|error| PromotionPreviewError::IsolationInvalid(error.to_string()))?;
        if document.isolation_contract.id == *isolation_id {
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
    }
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

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, PromotionPreviewError> {
    let mut file = File::open(path).map_err(|error| PromotionPreviewError::IsolationDocument {
        path: path.to_path_buf(),
        source: error.to_string(),
    })?;
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
    Ok(raw)
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
