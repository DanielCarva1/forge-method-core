//! Pure, deterministic planning for read-only governed-promotion previews.

use forge_core_contracts::{
    GovernedPromotionPreviewStatus, PromotionApplyEligibility, PromotionDiffEffect,
    PromotionDiffEntry, PromotionGap, PromotionGapCode, PromotionUnsupportedEffect,
    PromotionUnsupportedEffectKind, RepoPath,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionInventoryFile {
    pub relative_path: String,
    pub content_digest: String,
    pub byte_length: u64,
    pub metadata_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionInventoryDirectory {
    pub relative_path: String,
    pub metadata_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionDiffProjection {
    pub diff: Vec<PromotionDiffEntry>,
    pub write_set: Vec<RepoPath>,
    pub diff_digest: String,
    pub write_set_digest: String,
    pub predicted_result_regular_file_set_digest: String,
    pub unsupported_effects: Vec<PromotionUnsupportedEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionPlanningError {
    DuplicatePath(String),
    InvalidPath(String),
    Canonicalization(String),
}

impl std::fmt::Display for PromotionPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicatePath(path) => write!(formatter, "duplicate promotion path {path}"),
            Self::InvalidPath(path) => write!(formatter, "invalid promotion path {path}"),
            Self::Canonicalization(error) => {
                write!(formatter, "promotion canonicalization failed: {error}")
            }
        }
    }
}

impl std::error::Error for PromotionPlanningError {}

/// Derive the complete regular-file create/write/delete plan.
///
/// Input order is irrelevant. File metadata and empty-directory changes are
/// visible as unsupported effects and are never smuggled into the write set.
pub fn derive_promotion_diff(
    source_files: &[PromotionInventoryFile],
    destination_files: &[PromotionInventoryFile],
    source_directories: &[PromotionInventoryDirectory],
    destination_directories: &[PromotionInventoryDirectory],
) -> Result<PromotionDiffProjection, PromotionPlanningError> {
    let source = file_map(source_files)?;
    let destination = file_map(destination_files)?;
    let source_dirs = directory_map(source_directories)?;
    let destination_dirs = directory_map(destination_directories)?;

    let all_paths = source
        .keys()
        .chain(destination.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut diff = Vec::new();
    let mut unsupported_effects = Vec::new();
    for path in &all_paths {
        match (destination.get(path), source.get(path)) {
            (None, Some(after)) => {
                diff.push(PromotionDiffEntry {
                    path: RepoPath(path.clone()),
                    effect: PromotionDiffEffect::CreateRegularFile,
                    before_content_digest: None,
                    before_byte_length: None,
                    after_content_digest: Some(after.content_digest.clone()),
                    after_byte_length: Some(after.byte_length),
                    before_metadata_fingerprint: None,
                    after_metadata_fingerprint: Some(after.metadata_fingerprint.clone()),
                    destructive: false,
                });
                unsupported_effects.push(PromotionUnsupportedEffect {
                    path: RepoPath(path.clone()),
                    kind: PromotionUnsupportedEffectKind::FileMetadataCreate,
                    detail: format!(
                        "created regular-file metadata is explicit and unsupported (metadata={})",
                        after.metadata_fingerprint
                    ),
                });
            }
            (Some(before), None) => diff.push(PromotionDiffEntry {
                path: RepoPath(path.clone()),
                effect: PromotionDiffEffect::DeleteRegularFile,
                before_content_digest: Some(before.content_digest.clone()),
                before_byte_length: Some(before.byte_length),
                after_content_digest: None,
                after_byte_length: None,
                before_metadata_fingerprint: Some(before.metadata_fingerprint.clone()),
                after_metadata_fingerprint: None,
                destructive: true,
            }),
            (Some(before), Some(after)) => {
                if before.content_digest != after.content_digest
                    || before.byte_length != after.byte_length
                {
                    diff.push(PromotionDiffEntry {
                        path: RepoPath(path.clone()),
                        effect: PromotionDiffEffect::WriteRegularFile,
                        before_content_digest: Some(before.content_digest.clone()),
                        before_byte_length: Some(before.byte_length),
                        after_content_digest: Some(after.content_digest.clone()),
                        after_byte_length: Some(after.byte_length),
                        before_metadata_fingerprint: Some(before.metadata_fingerprint.clone()),
                        after_metadata_fingerprint: Some(after.metadata_fingerprint.clone()),
                        destructive: false,
                    });
                }
                if before.metadata_fingerprint != after.metadata_fingerprint {
                    unsupported_effects.push(PromotionUnsupportedEffect {
                        path: RepoPath(path.clone()),
                        kind: PromotionUnsupportedEffectKind::FileModeChange,
                        detail: format!(
                            "regular-file metadata differs ({} -> {}); preview does not apply metadata",
                            before.metadata_fingerprint, after.metadata_fingerprint
                        ),
                    });
                }
            }
            (None, None) => unreachable!("union path must exist in at least one map"),
        }
    }
    let all_directories = source_dirs
        .keys()
        .chain(destination_dirs.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for path in all_directories {
        let source_file = source.get(&path);
        let destination_file = destination.get(&path);
        let source_directory = source_dirs.get(&path);
        let destination_directory = destination_dirs.get(&path);
        if (source_file.is_some() && destination_directory.is_some())
            || (destination_file.is_some() && source_directory.is_some())
        {
            unsupported_effects.push(PromotionUnsupportedEffect {
                path: RepoPath(path),
                kind: PromotionUnsupportedEffectKind::ObjectTypeTransition,
                detail:
                    "regular-file/directory type transition requires an explicit later apply model"
                        .to_owned(),
            });
            continue;
        }
        match (destination_directory, source_directory) {
            (None, Some(after)) => unsupported_effects.push(PromotionUnsupportedEffect {
                path: RepoPath(path),
                kind: PromotionUnsupportedEffectKind::DirectoryCreate,
                detail: format!(
                    "directory creation topology is explicit and unsupported (metadata={})",
                    after.metadata_fingerprint
                ),
            }),
            (Some(before), None) => unsupported_effects.push(PromotionUnsupportedEffect {
                path: RepoPath(path),
                kind: PromotionUnsupportedEffectKind::DirectoryDelete,
                detail: format!(
                    "directory deletion topology is explicit and unsupported (metadata={})",
                    before.metadata_fingerprint
                ),
            }),
            (Some(before), Some(after))
                if before.metadata_fingerprint != after.metadata_fingerprint =>
            {
                unsupported_effects.push(PromotionUnsupportedEffect {
                    path: RepoPath(path),
                    kind: PromotionUnsupportedEffectKind::FileModeChange,
                    detail: format!(
                        "directory metadata differs ({} -> {}); preview does not apply metadata",
                        before.metadata_fingerprint, after.metadata_fingerprint
                    ),
                });
            }
            _ => {}
        }
    }

    diff.sort_by(|left, right| left.path.0.cmp(&right.path.0));
    unsupported_effects.sort_by(|left, right| {
        left.path
            .0
            .cmp(&right.path.0)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    let write_set = diff
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let diff_digest = promotion_domain_digest("promotion.diff.v1", &diff)?;
    let write_set_digest = promotion_domain_digest("promotion.write_set.v1", &write_set)?;
    let predicted_pairs = source
        .values()
        .map(|file| (file.relative_path.clone(), file.content_digest.clone()))
        .collect::<Vec<_>>();
    let predicted_result_regular_file_set_digest =
        promotion_domain_digest("promotion.predicted_regular_file_set.v1", &predicted_pairs)?;
    Ok(PromotionDiffProjection {
        diff,
        write_set,
        diff_digest,
        write_set_digest,
        predicted_result_regular_file_set_digest,
        unsupported_effects,
    })
}

pub struct PromotionReadinessInput<'a> {
    pub diff: &'a [PromotionDiffEntry],
    pub has_linked_claim: bool,
    pub ungoverned_paths: &'a [RepoPath],
    pub conflicting_paths: &'a [RepoPath],
    pub unsupported_effects: &'a [PromotionUnsupportedEffect],
    pub supporting_cooperative_evidence: usize,
    pub blocking_source_claim_refs: &'a [String],
    pub has_linked_claim_principal: bool,
    pub open_objective_uncertainties: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionReadinessEvaluation {
    pub status: GovernedPromotionPreviewStatus,
    pub apply_eligibility: PromotionApplyEligibility,
    pub unresolved_gaps: Vec<PromotionGap>,
}

/// Conservative readiness classification from kernel-derived facts.
///
/// No caller-authored coverage boolean is accepted by this API.
#[must_use]
pub fn evaluate_promotion_readiness(
    input: &PromotionReadinessInput<'_>,
) -> PromotionReadinessEvaluation {
    let mut gaps = Vec::new();
    if input.diff.is_empty() && input.unsupported_effects.is_empty() {
        gaps.push(gap(
            PromotionGapCode::NoChanges,
            "promotion.diff",
            "source and destination filesystem projection is identical",
        ));
    }
    if input.open_objective_uncertainties > 0 {
        gaps.push(gap(
            PromotionGapCode::OpenObjectiveUncertainties,
            "promotion.objective",
            format!(
                "accepted objective still carries {} open uncertainties",
                input.open_objective_uncertainties
            ),
        ));
    }
    if input.supporting_cooperative_evidence == 0 {
        gaps.push(gap(
            PromotionGapCode::MissingSupportingCooperativeEvidence,
            "promotion.evidence",
            "no current supporting cooperative evidence record is bound",
        ));
    }
    for claim_ref in input.blocking_source_claim_refs {
        gaps.push(gap(
            PromotionGapCode::SourceAssuranceClaimUnsatisfied,
            claim_ref,
            "source assurance claim is not verified or waived; same-owner cooperative evidence does not satisfy it",
        ));
    }
    if !input.has_linked_claim {
        gaps.push(gap(
            PromotionGapCode::MissingLinkedIsolationClaim,
            "promotion.source.isolation",
            "active isolation text has no linked claim and is not ownership proof",
        ));
    }
    if input.has_linked_claim && !input.has_linked_claim_principal {
        gaps.push(gap(
            PromotionGapCode::MissingLinkedClaimPrincipal,
            "promotion.source.linked_claim",
            "the current linked claim has no principal identity",
        ));
    }
    if !input.ungoverned_paths.is_empty() {
        gaps.push(gap(
            PromotionGapCode::UngovernedWriteSet,
            "promotion.write_set",
            format!(
                "{} write-set paths are not covered by the isolation's linked current claim",
                input.ungoverned_paths.len()
            ),
        ));
    }
    if !input.conflicting_paths.is_empty() {
        gaps.push(gap(
            PromotionGapCode::ConflictingClaim,
            "promotion.write_set",
            format!(
                "{} write-set paths conflict with another live claim",
                input.conflicting_paths.len()
            ),
        ));
    }
    if input.diff.iter().any(|entry| entry.destructive) {
        gaps.push(gap(
            PromotionGapCode::DestructiveDeleteRequiresSeparateAuthority,
            "promotion.diff",
            "one or more regular-file deletes require explicit later apply authority",
        ));
    }
    if !input.unsupported_effects.is_empty() {
        gaps.push(gap(
            PromotionGapCode::UnsupportedEffect,
            "promotion.unsupported_effects",
            format!(
                "{} filesystem effects are outside the regular-file promotion model",
                input.unsupported_effects.len()
            ),
        ));
    }
    gaps.sort_by(|left, right| {
        left.code
            .cmp(&right.code)
            .then_with(|| left.subject_ref.cmp(&right.subject_ref))
    });
    let status = if input.diff.is_empty() && input.unsupported_effects.is_empty() {
        GovernedPromotionPreviewStatus::NoChanges
    } else if gaps.is_empty() {
        GovernedPromotionPreviewStatus::Reviewable
    } else {
        GovernedPromotionPreviewStatus::Blocked
    };
    PromotionReadinessEvaluation {
        status,
        apply_eligibility: if status == GovernedPromotionPreviewStatus::Reviewable {
            PromotionApplyEligibility::EligibleLocalReversible
        } else {
            PromotionApplyEligibility::NotEligible
        },
        unresolved_gaps: gaps,
    }
}

pub fn promotion_domain_digest(
    domain: &'static str,
    value: &impl Serialize,
) -> Result<String, PromotionPlanningError> {
    #[derive(Serialize)]
    struct DomainSeparatedDigest<'a, T: Serialize + ?Sized> {
        domain: &'static str,
        payload: &'a T,
    }
    let bytes = serde_json_canonicalizer::to_vec(&DomainSeparatedDigest {
        domain,
        payload: value,
    })
    .map_err(|error| PromotionPlanningError::Canonicalization(error.to_string()))?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn file_map(
    files: &[PromotionInventoryFile],
) -> Result<BTreeMap<String, PromotionInventoryFile>, PromotionPlanningError> {
    let mut mapped = BTreeMap::new();
    for file in files {
        validate_relative_path(&file.relative_path)?;
        if mapped
            .insert(file.relative_path.clone(), file.clone())
            .is_some()
        {
            return Err(PromotionPlanningError::DuplicatePath(
                file.relative_path.clone(),
            ));
        }
    }
    Ok(mapped)
}

fn directory_map(
    directories: &[PromotionInventoryDirectory],
) -> Result<BTreeMap<String, PromotionInventoryDirectory>, PromotionPlanningError> {
    let mut mapped = BTreeMap::new();
    for directory in directories {
        validate_relative_path(&directory.relative_path)?;
        if mapped
            .insert(directory.relative_path.clone(), directory.clone())
            .is_some()
        {
            return Err(PromotionPlanningError::DuplicatePath(
                directory.relative_path.clone(),
            ));
        }
    }
    Ok(mapped)
}

fn validate_relative_path(path: &str) -> Result<(), PromotionPlanningError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(PromotionPlanningError::InvalidPath(path.to_owned()));
    }
    Ok(())
}

fn gap(
    code: PromotionGapCode,
    subject_ref: impl Into<String>,
    message: impl Into<String>,
) -> PromotionGap {
    PromotionGap {
        code,
        subject_ref: subject_ref.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, digest: &str, length: u64) -> PromotionInventoryFile {
        PromotionInventoryFile {
            relative_path: path.to_owned(),
            content_digest: digest.to_owned(),
            byte_length: length,
            metadata_fingerprint: "mode=a".to_owned(),
        }
    }

    #[test]
    fn diff_is_stable_and_complete_for_create_write_delete() {
        let source = vec![file("b.txt", "sha256:b", 2), file("a.txt", "sha256:new", 3)];
        let destination = vec![file("c.txt", "sha256:c", 4), file("a.txt", "sha256:old", 3)];
        let first = derive_promotion_diff(&source, &destination, &[], &[]).expect("diff");
        let mut reversed_source = source;
        reversed_source.reverse();
        let mut reversed_destination = destination;
        reversed_destination.reverse();
        let second =
            derive_promotion_diff(&reversed_source, &reversed_destination, &[], &[]).expect("diff");
        assert_eq!(first, second);
        assert_eq!(
            first
                .diff
                .iter()
                .map(|entry| entry.effect)
                .collect::<Vec<_>>(),
            vec![
                PromotionDiffEffect::WriteRegularFile,
                PromotionDiffEffect::CreateRegularFile,
                PromotionDiffEffect::DeleteRegularFile,
            ]
        );
        assert!(first.diff[2].destructive);
    }

    #[test]
    fn carried_assurance_gap_does_not_block_local_reversible_eligibility() {
        let changed = PromotionDiffEntry {
            path: RepoPath("src/lib.rs".to_owned()),
            effect: PromotionDiffEffect::WriteRegularFile,
            before_content_digest: Some("sha256:before".to_owned()),
            before_byte_length: Some(1),
            after_content_digest: Some("sha256:after".to_owned()),
            after_byte_length: Some(1),
            before_metadata_fingerprint: Some("mode=a".to_owned()),
            after_metadata_fingerprint: Some("mode=a".to_owned()),
            destructive: false,
        };
        let evaluation = evaluate_promotion_readiness(&PromotionReadinessInput {
            diff: &[changed],
            has_linked_claim: true,
            ungoverned_paths: &[],
            conflicting_paths: &[],
            unsupported_effects: &[],
            supporting_cooperative_evidence: 1,
            blocking_source_claim_refs: &[],
            has_linked_claim_principal: true,
            open_objective_uncertainties: 0,
        });
        assert_eq!(
            evaluation.status,
            GovernedPromotionPreviewStatus::Reviewable
        );
        assert_eq!(
            evaluation.apply_eligibility,
            PromotionApplyEligibility::EligibleLocalReversible
        );
        assert!(evaluation.unresolved_gaps.is_empty());
    }

    #[test]
    fn disproven_source_assurance_remains_blocking() {
        let changed = PromotionDiffEntry {
            path: RepoPath("src/lib.rs".to_owned()),
            effect: PromotionDiffEffect::WriteRegularFile,
            before_content_digest: Some("sha256:before".to_owned()),
            before_byte_length: Some(1),
            after_content_digest: Some("sha256:after".to_owned()),
            after_byte_length: Some(1),
            before_metadata_fingerprint: Some("mode=a".to_owned()),
            after_metadata_fingerprint: Some("mode=a".to_owned()),
            destructive: false,
        };
        let evaluation = evaluate_promotion_readiness(&PromotionReadinessInput {
            diff: &[changed],
            has_linked_claim: true,
            has_linked_claim_principal: true,
            ungoverned_paths: &[],
            conflicting_paths: &[],
            unsupported_effects: &[],
            supporting_cooperative_evidence: 1,
            blocking_source_claim_refs: &["claim.source.runtime".to_owned()],
            open_objective_uncertainties: 0,
        });
        assert_eq!(evaluation.status, GovernedPromotionPreviewStatus::Blocked);
    }

    #[test]
    fn digest_domains_do_not_collide_for_equal_payloads() {
        let payload = vec!["same"];
        let left = promotion_domain_digest("promotion.diff.v1", &payload).expect("left");
        let right = promotion_domain_digest("promotion.write_set.v1", &payload).expect("right");
        assert_ne!(left, right);
    }

    #[test]
    fn file_directory_type_transition_is_explicit_and_blocked() {
        let source_file = file("shape", "sha256:file", 4);
        let destination_dirs = vec![PromotionInventoryDirectory {
            relative_path: "shape".to_owned(),
            metadata_fingerprint: "mode=a".to_owned(),
        }];
        let projection =
            derive_promotion_diff(&[source_file], &[], &[], &destination_dirs).expect("projection");
        assert!(projection.unsupported_effects.iter().any(|effect| {
            effect.path == RepoPath("shape".to_owned())
                && effect.kind == PromotionUnsupportedEffectKind::ObjectTypeTransition
        }));
    }

    #[test]
    fn created_file_metadata_is_explicit_and_blocked() {
        let mut source_file = file("bin/tool", "sha256:new", 4);
        source_file.metadata_fingerprint = "mode=100755;uid=1000;gid=1000".to_owned();
        let projection = derive_promotion_diff(&[source_file], &[], &[], &[]).expect("projection");
        assert_eq!(
            projection.diff[0].effect,
            PromotionDiffEffect::CreateRegularFile
        );
        assert!(projection.unsupported_effects.iter().any(|effect| {
            effect.path == RepoPath("bin/tool".to_owned())
                && effect.kind == PromotionUnsupportedEffectKind::FileMetadataCreate
        }));
        let readiness = evaluate_promotion_readiness(&PromotionReadinessInput {
            diff: &projection.diff,
            has_linked_claim: true,
            ungoverned_paths: &[],
            conflicting_paths: &[],
            unsupported_effects: &projection.unsupported_effects,
            supporting_cooperative_evidence: 1,
            blocking_source_claim_refs: &[],
            has_linked_claim_principal: true,
            open_objective_uncertainties: 0,
        });
        assert_eq!(readiness.status, GovernedPromotionPreviewStatus::Blocked);
    }

    #[test]
    fn metadata_and_empty_directory_effects_are_explicit() {
        let mut source_file = file("src/lib.rs", "sha256:same", 4);
        source_file.metadata_fingerprint = "mode=b".to_owned();
        let destination_file = file("src/lib.rs", "sha256:same", 4);
        let source_dirs = vec![
            PromotionInventoryDirectory {
                relative_path: "src".to_owned(),
                metadata_fingerprint: "mode=a".to_owned(),
            },
            PromotionInventoryDirectory {
                relative_path: "empty".to_owned(),
                metadata_fingerprint: "mode=a".to_owned(),
            },
        ];
        let destination_dirs = vec![PromotionInventoryDirectory {
            relative_path: "src".to_owned(),
            metadata_fingerprint: "mode=a".to_owned(),
        }];
        let projection = derive_promotion_diff(
            &[source_file],
            &[destination_file],
            &source_dirs,
            &destination_dirs,
        )
        .expect("diff");
        assert!(projection.diff.is_empty());
        assert_eq!(projection.unsupported_effects.len(), 2);
        let readiness = evaluate_promotion_readiness(&PromotionReadinessInput {
            diff: &projection.diff,
            has_linked_claim: true,
            ungoverned_paths: &[],
            conflicting_paths: &[],
            unsupported_effects: &projection.unsupported_effects,
            supporting_cooperative_evidence: 1,
            blocking_source_claim_refs: &[],
            has_linked_claim_principal: true,
            open_objective_uncertainties: 0,
        });
        assert_eq!(readiness.status, GovernedPromotionPreviewStatus::Blocked);
    }
}
