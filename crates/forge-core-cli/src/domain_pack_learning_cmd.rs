//! Agent-facing P6c Domain Pack learning and reviewed-registry journey.
#![allow(clippy::similar_names)] // `reviewer` and `reviewed` are distinct protocol terms.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use forge_core_authority::{
    domain_pack_reviewed_registry_digest, domain_pack_reviewer_registry_digest,
    verify_domain_pack_promotion_authorization, AnchoredReviewedDomainPackRegistrySnapshot,
    DomainPackPromotionExpectedContext, DomainPackReviewerRegistryAnchor,
    ReviewedDomainPackRegistryAnchor,
};
use forge_core_contracts::{
    CliEnvelope, DomainPackIndependentReviewDocument, DomainPackLearningConflictDocument,
    DomainPackLocalLearningCandidateDocument, DomainPackPromotionAuthorizationDocument,
    DomainPackPromotionDecisionDocument, DomainPackPromotionDossierDocument,
    DomainPackReviewedRegistryDocument, DomainPackReviewerRegistryDocument, StableId,
};
use forge_core_decisions::domain_pack_learning::{
    evaluate_domain_pack_promotion, evaluate_domain_pack_reviewed_registry_evolution,
    DomainPackPromotionEvaluationInput, DomainPackReviewedRegistryEvolutionInput,
};
use forge_core_domain_pack_learning_store::{
    candidate_self_digest, capture_local_learning, learning_store_status,
};
use forge_core_store::{
    retained_crash_replace::reconcile_file_crash_safe_under_owned_lock,
    OwnedRetainedCrashReplaceRead, OwnedRetainedCrashReplaceSession,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;

const ACK: &str = "I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE";
const LOCK_PATH: &str = ".forge-domain-pack-learning-anchor.lock";
const HEAD_PATH: &str = ".forge-domain-pack-learning-anchor.yaml";
const HEAD_SCHEMA: &str = "forge-domain-pack-learning-anchor-v1";
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewerHead {
    registry_id: StableId,
    audience: String,
    generation: u64,
    registry_digest: String,
    full_digest: String,
    trust_policy_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedHead {
    registry_id: StableId,
    audience: String,
    generation: u64,
    registry_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LearningAnchorHead {
    schema_version: String,
    reviewer: ReviewerHead,
    reviewed: ReviewedHead,
}

struct LockedHeads {
    root: PathBuf,
    root_identity: crate::io_util::RetainedDirectoryIdentity,
    reconciliation: Option<OwnedRetainedCrashReplaceSession>,
    exact_read: Option<OwnedRetainedCrashReplaceRead>,
}

#[allow(dead_code)]
/// Protected learning-head values captured with the exact raw anchor bytes.
pub(crate) struct ReviewedLearningHeadProjection {
    reviewer_generation: u64,
    reviewed_generation: u64,
    reviewer_registry_digest: String,
    reviewed_registry_digest: String,
}

#[allow(dead_code)]
impl ReviewedLearningHeadProjection {
    pub(crate) const fn reviewer_generation(&self) -> u64 {
        self.reviewer_generation
    }

    pub(crate) const fn reviewed_generation(&self) -> u64 {
        self.reviewed_generation
    }

    pub(crate) fn reviewer_registry_digest(&self) -> &str {
        &self.reviewer_registry_digest
    }

    pub(crate) fn reviewed_registry_digest(&self) -> &str {
        &self.reviewed_registry_digest
    }
}

#[allow(dead_code)]
/// Retains the learning-anchor OS lock while a fresh opaque reviewed snapshot
/// and its exact public source bytes are consumed.
pub(crate) struct LockedReviewedSnapshot {
    snapshot: AnchoredReviewedDomainPackRegistrySnapshot,
    head: ReviewedLearningHeadProjection,
    raw_anchor: Vec<u8>,
    raw_anchor_sha256: String,
    raw_reviewer_registry: Vec<u8>,
    raw_reviewed_registry: Vec<u8>,
    _locked: LockedHeads,
}

#[allow(dead_code)]
impl LockedReviewedSnapshot {
    pub(crate) const fn snapshot(&self) -> &AnchoredReviewedDomainPackRegistrySnapshot {
        &self.snapshot
    }

    pub(crate) const fn head(&self) -> &ReviewedLearningHeadProjection {
        &self.head
    }

    pub(crate) fn raw_anchor(&self) -> &[u8] {
        &self.raw_anchor
    }

    pub(crate) fn raw_anchor_sha256(&self) -> &str {
        &self.raw_anchor_sha256
    }

    pub(crate) fn raw_reviewer_registry(&self) -> &[u8] {
        &self.raw_reviewer_registry
    }

    pub(crate) fn raw_reviewed_registry(&self) -> &[u8] {
        &self.raw_reviewed_registry
    }
}

#[derive(Default)]
struct LearningFlags {
    state_root: Option<PathBuf>,
    project_root: Option<PathBuf>,
    operator_root: Option<PathBuf>,
    candidate_files: Vec<PathBuf>,
    review_files: Vec<PathBuf>,
    conflict_files: Vec<PathBuf>,
    dossier_file: Option<PathBuf>,
    decision_file: Option<PathBuf>,
    authorization_file: Option<PathBuf>,
    reviewer_registry_file: Option<PathBuf>,
    reviewed_registry_file: Option<PathBuf>,
    proposed_registry_file: Option<PathBuf>,
    proposed_reviewer_registry_file: Option<PathBuf>,
    acknowledgement: Option<String>,
    want_json: bool,
}

/// Dispatch `forge-core domain-pack learning ...`.
///
/// # Errors
/// Returns a typed CLI error for invalid arguments, untrusted roots, malformed
/// bounded documents, failed governance, or crash-safe persistence failure.
pub fn run_domain_pack_learning_command(args: &[String]) -> Result<(), ExitError> {
    match args.first().map_or("--help", String::as_str) {
        "capture" => run_capture(&args[1..]),
        "status" => run_status(&args[1..]),
        "evaluate" => run_evaluate(&args[1..], false),
        "conflict-check" => run_evaluate(&args[1..], true),
        "trust-provision" => run_trust_provision(&args[1..]),
        "reviewer-rotate" => run_reviewer_rotate(&args[1..]),
        "registry-check" => run_registry_check(&args[1..]),
        "promote" => run_promote(&args[1..]),
        "--help" | "-h" | "help" => {
            println!("{}", usage());
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core domain-pack learning: unknown subcommand '{other}'\n{}",
            usage()
        ))),
    }
}

fn run_capture(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    let state = required_path(flags.state_root.as_deref(), "--state-root")?;
    let candidate = exactly_one(&flags.candidate_files, "--candidate-file")?;
    let raw = read_bounded(candidate, "local learning candidate")?;
    // Compute before capture so agents receive the expected digest even when
    // authored bytes fail the store's exact self-digest gate.
    let typed: DomainPackLocalLearningCandidateDocument = parse(&raw, candidate)?;
    let computed = candidate_self_digest(&typed).map_err(store_error)?;
    let receipt = capture_local_learning(state, &raw).map_err(store_error)?;
    emit(
        "domain-pack learning capture",
        serde_json::json!({
            "receipt": receipt, "computed_candidate_digest": computed,
            "boundary": "captured local observation remains non_authoritative and cannot activate"
        }),
        flags.want_json,
    )
}

fn run_status(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    let state = required_path(flags.state_root.as_deref(), "--state-root")?;
    let status = learning_store_status(state).map_err(store_error)?;
    emit(
        "domain-pack learning status",
        serde_json::json!({
            "store": status,
            "boundary": "verified storage integrity is not semantic review or activation authority"
        }),
        flags.want_json,
    )
}

fn run_evaluate(args: &[String], conflicts_only: bool) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    let dossier_path = required_path(flags.dossier_file.as_deref(), "--dossier-file")?;
    let dossier: DomainPackPromotionDossierDocument = read_typed(dossier_path)?;
    let candidates = read_many::<DomainPackLocalLearningCandidateDocument>(&flags.candidate_files)?;
    let reviews = read_many::<DomainPackIndependentReviewDocument>(&flags.review_files)?;
    let conflicts = read_many::<DomainPackLearningConflictDocument>(&flags.conflict_files)?;
    let evaluation = evaluate_domain_pack_promotion(&DomainPackPromotionEvaluationInput {
        dossier: &dossier,
        candidates: &candidates,
        independent_reviews: &reviews,
        conflicts: &conflicts,
    });
    let command = if conflicts_only {
        "domain-pack learning conflict-check"
    } else {
        "domain-pack learning evaluate"
    };
    emit(
        command,
        serde_json::json!({
            "evaluation": evaluation,
            "boundary": "pure non_authoritative guidance; only signed promotion plus anchored registry can activate"
        }),
        flags.want_json,
    )
}

fn run_trust_provision(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    if flags.acknowledgement.as_deref() != Some(ACK) {
        return Err(ExitError::invalid_value(format!(
            "domain-pack learning trust-provision requires --operator-acknowledge-trust-on-first-use {ACK}"
        )));
    }
    let operator_root = external_operator_root(&flags)?;
    let reviewer_path = required_path(
        flags.reviewer_registry_file.as_deref(),
        "--reviewer-registry-file",
    )?;
    let reviewed_path = required_path(
        flags.reviewed_registry_file.as_deref(),
        "--reviewed-registry-file",
    )?;
    require_direct_file(reviewer_path, &operator_root, "--reviewer-registry-file")?;
    require_direct_file(reviewed_path, &operator_root, "--reviewed-registry-file")?;
    let reviewer: DomainPackReviewerRegistryDocument = read_typed(reviewer_path)?;
    let reviewed: DomainPackReviewedRegistryDocument = read_typed(reviewed_path)?;
    let reviewer_digest =
        domain_pack_reviewer_registry_digest(&reviewer).map_err(authority_error)?;
    let reviewer_full = canonical_digest(&reviewer)?;
    let reviewer_value = &reviewer.domain_pack_reviewer_registry;
    let reviewer_anchor = DomainPackReviewerRegistryAnchor::from_operator_protected_genesis(
        reviewer.clone(),
        &reviewer_value.trust_policy_digest,
        &reviewer_full,
    )
    .map_err(authority_error)?;
    let reviewed_digest =
        domain_pack_reviewed_registry_digest(&reviewed).map_err(authority_error)?;
    let now = trusted_now()?;
    let _ = ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
        &reviewer_anchor,
        reviewed.clone(),
        &reviewed_digest,
        now,
    )
    .map_err(authority_error)?;
    let locked = lock_heads(&operator_root)?;
    if locked
        .reconciliation
        .as_ref()
        .is_some_and(|reconciliation| reconciliation.raw_bytes().is_some())
    {
        return Err(ExitError::conflict(
            "domain-pack learning trust is already provisioned; refusing silent trust-root replacement",
        ));
    }
    let reviewer_head = ReviewerHead {
        registry_id: reviewer_value.registry_id.clone(),
        audience: reviewer_value.audience.clone(),
        generation: reviewer_value.generation,
        registry_digest: reviewer_digest.clone(),
        full_digest: reviewer_full,
        trust_policy_digest: reviewer_value.trust_policy_digest.clone(),
    };
    let reviewed_value = &reviewed.domain_pack_reviewed_registry;
    let reviewed_head = ReviewedHead {
        registry_id: reviewed_value.registry_id.clone(),
        audience: reviewed_value.audience.clone(),
        generation: reviewed_value.generation,
        registry_digest: reviewed_digest.clone(),
    };
    persist_head(
        locked,
        &LearningAnchorHead {
            schema_version: HEAD_SCHEMA.to_owned(),
            reviewer: reviewer_head,
            reviewed: reviewed_head,
        },
    )?;
    emit(
        "domain-pack learning trust-provision",
        serde_json::json!({
            "operator_root": operator_root, "reviewer_registry_digest": reviewer_digest,
            "reviewed_registry_digest": reviewed_digest,
            "boundary": "explicit operator-approved semantic-review trust roots; no project activation"
        }),
        flags.want_json,
    )
}

fn run_reviewer_rotate(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    let operator_root = external_operator_root(&flags)?;
    let current_path = required_path(
        flags.reviewer_registry_file.as_deref(),
        "--reviewer-registry-file",
    )?;
    let proposed_path = required_path(
        flags.proposed_reviewer_registry_file.as_deref(),
        "--proposed-reviewer-registry-file",
    )?;
    require_direct_file(current_path, &operator_root, "--reviewer-registry-file")?;
    require_direct_file(
        proposed_path,
        &operator_root,
        "--proposed-reviewer-registry-file",
    )?;
    let current: DomainPackReviewerRegistryDocument = read_typed(current_path)?;
    let proposed: DomainPackReviewerRegistryDocument = read_typed(proposed_path)?;
    let locked = lock_heads(&operator_root)?;
    let mut head = load_head_for_update(&locked)?;
    let mut anchor = restore_reviewer_anchor(current, &head.reviewer)?;
    let expected = anchor.version();
    let audit = anchor
        .compare_and_advance(&expected, proposed.clone(), trusted_now()?)
        .map_err(authority_error)?;
    let value = &proposed.domain_pack_reviewer_registry;
    head.reviewer = ReviewerHead {
        registry_id: value.registry_id.clone(),
        audience: value.audience.clone(),
        generation: value.generation,
        registry_digest: audit.registry_digest.clone(),
        full_digest: canonical_digest(&proposed)?,
        trust_policy_digest: value.trust_policy_digest.clone(),
    };
    persist_head(locked, &head)?;
    emit(
        "domain-pack learning reviewer-rotate",
        serde_json::json!({
            "generation": value.generation,
            "reviewer_registry_digest": audit.registry_digest,
            "reviewed_registry_digest": head.reviewed.registry_digest,
            "boundary": "predecessor-signed reviewer trust rotation persisted atomically; reviewed snapshots require fresh signatures under this head"
        }),
        flags.want_json,
    )
}

fn run_registry_check(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    let operator_root = external_operator_root(&flags)?;
    let reviewer_path = required_path(
        flags.reviewer_registry_file.as_deref(),
        "--reviewer-registry-file",
    )?;
    let reviewed_path = required_path(
        flags.reviewed_registry_file.as_deref(),
        "--reviewed-registry-file",
    )?;
    require_direct_file(reviewer_path, &operator_root, "--reviewer-registry-file")?;
    require_direct_file(reviewed_path, &operator_root, "--reviewed-registry-file")?;
    let mut locked = lock_heads(&operator_root)?;
    let head = load_head(&mut locked)?;
    let reviewer: DomainPackReviewerRegistryDocument = read_typed(reviewer_path)?;
    let reviewed: DomainPackReviewedRegistryDocument = read_typed(reviewed_path)?;
    let reviewer_anchor = restore_reviewer_anchor(reviewer, &head.reviewer)?;
    let now = trusted_now()?;
    let mut reviewed_anchor =
        restore_reviewed_anchor(&reviewer_anchor, reviewed.clone(), &head.reviewed, now)?;
    let verified = reviewed_anchor
        .verify_exact_replay(&reviewer_anchor, reviewed.clone(), now)
        .map_err(authority_error)?;
    let evolution = evaluate_domain_pack_reviewed_registry_evolution(
        &DomainPackReviewedRegistryEvolutionInput {
            current: Some(&reviewed),
            proposed: &reviewed,
            competing_heads: &[],
        },
    );
    emit(
        "domain-pack learning registry-check",
        serde_json::json!({
            "reviewer_registry_digest": verified.reviewer_registry_digest(),
            "reviewed_registry_digest": verified.registry_digest(), "generation": head.reviewed.generation,
            "evolution": evolution,
            "boundary": "fresh exact cryptographic replay of operator-protected reviewed knowledge"
        }),
        flags.want_json,
    )
}

/// Load and freshly verify the exact reviewed snapshot while retaining the
/// combined learning-anchor lock for the caller's complete lifecycle TCB
/// transaction. Callers acquire the supply-chain anchor first, establishing a
/// single supply-then-learning lock order across the CLI.
pub(crate) fn lock_reviewed_snapshot_for_lifecycle(
    operator_root: &Path,
    reviewer_registry_file: &Path,
    reviewed_registry_file: &Path,
    verified_at_unix: u64,
) -> Result<LockedReviewedSnapshot, ExitError> {
    require_direct_file(
        reviewer_registry_file,
        operator_root,
        "--reviewer-registry-file",
    )?;
    require_direct_file(
        reviewed_registry_file,
        operator_root,
        "--reviewed-registry-file",
    )?;
    let mut locked = lock_heads(operator_root)?;
    let raw_anchor = snapshot_learning_head(&mut locked)?.ok_or_else(|| {
        ExitError::invalid_value(
            "learning trust anchors are not provisioned; run trust-provision first",
        )
    })?;
    let anchor_path = operator_root.join(HEAD_PATH);
    let head: LearningAnchorHead = parse(&raw_anchor, &anchor_path)?;
    if head.schema_version != HEAD_SCHEMA {
        return Err(ExitError::invalid_value(
            "unsupported learning anchor head schema",
        ));
    }
    let reviewer_file = reviewer_registry_file
        .strip_prefix(operator_root)
        .map_err(|error| ExitError::failed(format!("reviewer registry path: {error}")))?;
    let reviewed_file = reviewed_registry_file
        .strip_prefix(operator_root)
        .map_err(|error| ExitError::failed(format!("reviewed registry path: {error}")))?;
    let raw_reviewer_registry = locked
        .root_identity
        .read_direct_file_bounded(reviewer_file, MAX_DOCUMENT_BYTES)
        .map_err(|error| {
            ExitError::failed(format!(
                "cannot read {}: {error}",
                reviewer_registry_file.display()
            ))
        })?;
    let raw_reviewed_registry = locked
        .root_identity
        .read_direct_file_bounded(reviewed_file, MAX_DOCUMENT_BYTES)
        .map_err(|error| {
            ExitError::failed(format!(
                "cannot read {}: {error}",
                reviewed_registry_file.display()
            ))
        })?;
    let reviewer: DomainPackReviewerRegistryDocument =
        parse(&raw_reviewer_registry, reviewer_registry_file)?;
    let reviewed: DomainPackReviewedRegistryDocument =
        parse(&raw_reviewed_registry, reviewed_registry_file)?;
    let reviewer_anchor = restore_reviewer_anchor(reviewer, &head.reviewer)?;
    let mut reviewed_anchor = restore_reviewed_anchor(
        &reviewer_anchor,
        reviewed.clone(),
        &head.reviewed,
        verified_at_unix,
    )?;
    let snapshot = reviewed_anchor
        .verify_exact_replay(&reviewer_anchor, reviewed, verified_at_unix)
        .map_err(authority_error)?;
    let head_projection = ReviewedLearningHeadProjection {
        reviewer_generation: head.reviewer.generation,
        reviewed_generation: head.reviewed.generation,
        reviewer_registry_digest: head.reviewer.registry_digest,
        reviewed_registry_digest: head.reviewed.registry_digest,
    };
    let raw_anchor_sha256 = format!("sha256:{:x}", Sha256::digest(&raw_anchor));
    locked
        .root_identity
        .validate()
        .map_err(|error| ExitError::failed(format!("learning root changed: {error}")))?;
    Ok(LockedReviewedSnapshot {
        snapshot,
        head: head_projection,
        raw_anchor,
        raw_anchor_sha256,
        raw_reviewer_registry,
        raw_reviewed_registry,
        _locked: locked,
    })
}

#[allow(clippy::too_many_lines)]
fn run_promote(args: &[String]) -> Result<(), ExitError> {
    let flags = parse_flags(args)?;
    if flags.candidate_files.is_empty() {
        return Err(ExitError::usage(
            "domain-pack learning promote requires --candidate-file for the exact promotion graph",
        ));
    }
    let operator_root = external_operator_root(&flags)?;
    let reviewer_path = required_path(
        flags.reviewer_registry_file.as_deref(),
        "--reviewer-registry-file",
    )?;
    let current_path = required_path(
        flags.reviewed_registry_file.as_deref(),
        "--reviewed-registry-file",
    )?;
    let proposed_path = required_path(
        flags.proposed_registry_file.as_deref(),
        "--proposed-registry-file",
    )?;
    for (path, label) in [
        (reviewer_path, "--reviewer-registry-file"),
        (current_path, "--reviewed-registry-file"),
        (proposed_path, "--proposed-registry-file"),
    ] {
        require_direct_file(path, &operator_root, label)?;
    }
    let dossier_path = required_path(flags.dossier_file.as_deref(), "--dossier-file")?;
    let decision_path = required_path(flags.decision_file.as_deref(), "--decision-file")?;
    let authorization_path =
        required_path(flags.authorization_file.as_deref(), "--authorization-file")?;
    if flags.review_files.is_empty() {
        return Err(ExitError::usage(
            "domain-pack learning promote requires --review-file at least twice",
        ));
    }
    let reviewer: DomainPackReviewerRegistryDocument = read_typed(reviewer_path)?;
    let current: DomainPackReviewedRegistryDocument = read_typed(current_path)?;
    let proposed: DomainPackReviewedRegistryDocument = read_typed(proposed_path)?;
    let dossier: DomainPackPromotionDossierDocument = read_typed(dossier_path)?;
    let decision: DomainPackPromotionDecisionDocument = read_typed(decision_path)?;
    let authorization: DomainPackPromotionAuthorizationDocument = read_typed(authorization_path)?;
    let candidates = read_many::<DomainPackLocalLearningCandidateDocument>(&flags.candidate_files)?;
    let reviews = read_many::<DomainPackIndependentReviewDocument>(&flags.review_files)?;
    let conflicts = read_many::<DomainPackLearningConflictDocument>(&flags.conflict_files)?;
    let locked = lock_heads(&operator_root)?;
    let mut head = load_head_for_update(&locked)?;
    let reviewer_anchor = restore_reviewer_anchor(reviewer, &head.reviewer)?;
    let now = trusted_now()?;
    let mut reviewed_anchor =
        restore_reviewed_anchor(&reviewer_anchor, current.clone(), &head.reviewed, now)?;
    let capability = verify_domain_pack_promotion_authorization(
        &reviewer_anchor,
        &authorization,
        DomainPackPromotionExpectedContext {
            dossier: &dossier,
            decision: &decision,
            candidates: &candidates,
            independent_reviews: &reviews,
            conflicts: &conflicts,
            current_reviewed_registry: &current,
            proposed_reviewed_registry: &proposed,
            verified_at_unix: now,
        },
        &head.reviewer.audience,
    )
    .map_err(authority_error)?;
    let expected = reviewed_anchor.version();
    let anchored = reviewed_anchor
        .compare_and_advance(&expected, &reviewer_anchor, capability, now)
        .map_err(authority_error)?;
    let value = &anchored.registry().domain_pack_reviewed_registry;
    let next_head = ReviewedHead {
        registry_id: value.registry_id.clone(),
        audience: value.audience.clone(),
        generation: value.generation,
        registry_digest: anchored.registry_digest().to_owned(),
    };
    head.reviewed = next_head.clone();
    persist_head(locked, &head)?;
    emit(
        "domain-pack learning promote",
        serde_json::json!({
            "generation": next_head.generation, "reviewer_registry_digest": anchored.reviewer_registry_digest(),
            "reviewed_registry_digest": anchored.registry_digest(), "authorization": anchored.authorization_audit(),
            "boundary": "opaque dual-reviewed authority consumed under retained operator lock and monotonic CAS"
        }),
        flags.want_json,
    )
}

fn parse_flags(args: &[String]) -> Result<LearningFlags, ExitError> {
    let mut flags = LearningFlags {
        want_json: true,
        ..LearningFlags::default()
    };
    let mut index = 0usize;
    while index < args.len() {
        let flag = args[index].as_str();
        if flag == "--json" {
            flags.want_json = true;
            index += 1;
            continue;
        }
        if matches!(flag, "--no-json" | "--text") {
            flags.want_json = false;
            index += 1;
            continue;
        }
        if matches!(flag, "--help" | "-h") {
            return Err(ExitError::usage(usage()));
        }
        index += 1;
        let value = args
            .get(index)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| ExitError::usage(usage()))?;
        match flag {
            "--state-root" => flags.state_root = Some(PathBuf::from(value)),
            "--project-root" => flags.project_root = Some(PathBuf::from(value)),
            "--operator-root" => flags.operator_root = Some(PathBuf::from(value)),
            "--candidate-file" => flags.candidate_files.push(PathBuf::from(value)),
            "--review-file" => flags.review_files.push(PathBuf::from(value)),
            "--conflict-file" => flags.conflict_files.push(PathBuf::from(value)),
            "--dossier-file" => flags.dossier_file = Some(PathBuf::from(value)),
            "--decision-file" => flags.decision_file = Some(PathBuf::from(value)),
            "--authorization-file" => flags.authorization_file = Some(PathBuf::from(value)),
            "--reviewer-registry-file" => flags.reviewer_registry_file = Some(PathBuf::from(value)),
            "--reviewed-registry-file" => flags.reviewed_registry_file = Some(PathBuf::from(value)),
            "--proposed-registry-file" => flags.proposed_registry_file = Some(PathBuf::from(value)),
            "--proposed-reviewer-registry-file" => {
                flags.proposed_reviewer_registry_file = Some(PathBuf::from(value));
            }
            "--operator-acknowledge-trust-on-first-use" => {
                flags.acknowledgement = Some(value.clone());
            }
            _ => return Err(ExitError::usage(usage())),
        }
        index += 1;
    }
    Ok(flags)
}

fn required_path<'a>(value: Option<&'a Path>, flag: &str) -> Result<&'a Path, ExitError> {
    value.ok_or_else(|| ExitError::usage(format!("missing required {flag}\n{}", usage())))
}

fn exactly_one<'a>(values: &'a [PathBuf], flag: &str) -> Result<&'a Path, ExitError> {
    if values.len() == 1 {
        Ok(&values[0])
    } else {
        Err(ExitError::usage(format!("exactly one {flag} is required")))
    }
}

fn external_operator_root(flags: &LearningFlags) -> Result<PathBuf, ExitError> {
    let _ = required_path(flags.project_root.as_deref(), "--project-root")?;
    let _ = required_path(flags.state_root.as_deref(), "--state-root")?;
    let requested = required_path(flags.operator_root.as_deref(), "--operator-root")?;
    let metadata = fs::symlink_metadata(requested)
        .map_err(|error| ExitError::failed(format!("cannot inspect --operator-root: {error}")))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(
            "--operator-root must be a real directory, not a link",
        ));
    }
    let root = fs::canonicalize(requested).map_err(|error| {
        ExitError::failed(format!("cannot canonicalize --operator-root: {error}"))
    })?;
    for (candidate, label) in [
        (&flags.project_root, "--project-root"),
        (&flags.state_root, "--state-root"),
    ] {
        if let Some(path) = candidate {
            let canonical = fs::canonicalize(path).map_err(|error| {
                ExitError::failed(format!("cannot canonicalize {label}: {error}"))
            })?;
            if canonical.starts_with(&root) || root.starts_with(&canonical) {
                return Err(ExitError::invalid_value(format!(
                    "--operator-root must be disjoint from {label}"
                )));
            }
        }
    }
    Ok(root)
}

fn require_direct_file(path: &Path, operator_root: &Path, label: &str) -> Result<(), ExitError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ExitError::failed(format!("cannot inspect {label}: {error}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ExitError::invalid_value(format!(
            "{label} must be a regular non-link file"
        )));
    }
    let canonical = fs::canonicalize(path).map_err(|error| ExitError::failed(error.to_string()))?;
    if canonical.parent() != Some(operator_root) {
        return Err(ExitError::invalid_value(format!(
            "{label} must be a direct child of --operator-root"
        )));
    }
    Ok(())
}

fn lock_heads(root: &Path) -> Result<LockedHeads, ExitError> {
    let retained_root = forge_core_store::RetainedEffectStoreRoot::acquire(root)
        .map_err(|error| ExitError::failed(format!("cannot bind learning effect root: {error}")))?;
    let root_identity = crate::io_util::RetainedDirectoryIdentity::capture(root)
        .map_err(|error| ExitError::failed(format!("cannot bind learning root: {error}")))?;
    let lock = crate::io_util::acquire_effect_store_lock_retained(&retained_root, LOCK_PATH)
        .map_err(|error| ExitError::failed(format!("cannot lock learning anchors: {error}")))?;
    root_identity
        .validate()
        .map_err(|error| ExitError::failed(format!("learning root changed: {error}")))?;
    let reconciliation =
        reconcile_file_crash_safe_under_owned_lock(lock, Path::new(HEAD_PATH), MAX_DOCUMENT_BYTES)
            .map_err(|error| {
                ExitError::failed(format!("cannot recover learning anchor: {error}"))
            })?;
    root_identity
        .validate()
        .map_err(|error| ExitError::failed(format!("learning root changed: {error}")))?;
    Ok(LockedHeads {
        root: root.to_path_buf(),
        root_identity,
        reconciliation: Some(reconciliation),
        exact_read: None,
    })
}

fn snapshot_learning_head(locked: &mut LockedHeads) -> Result<Option<Vec<u8>>, ExitError> {
    if let Some(read) = locked.exact_read.as_mut() {
        read.revalidate().map_err(|error| {
            ExitError::conflict(format!(
                "{HEAD_PATH} changed after locked recovery: {error}"
            ))
        })?;
        return Ok(Some(read.raw_bytes().to_vec()));
    }
    let present = locked
        .reconciliation
        .as_ref()
        .ok_or_else(|| ExitError::failed("learning-head reconciliation authority was consumed"))?
        .raw_bytes()
        .is_some();
    if !present {
        let current = locked
            .root_identity
            .read_optional_direct_file_bounded(Path::new(HEAD_PATH), MAX_DOCUMENT_BYTES)
            .map_err(|error| ExitError::failed(format!("cannot inspect {HEAD_PATH}: {error}")))?;
        if current.is_some() {
            return Err(ExitError::conflict(format!(
                "{HEAD_PATH} appeared after locked recovery"
            )));
        }
        return Ok(None);
    }
    let session = locked
        .reconciliation
        .take()
        .ok_or_else(|| ExitError::failed("learning-head reconciliation authority was consumed"))?;
    let mut read = session
        .read_exact()
        .map_err(|error| {
            ExitError::conflict(format!(
                "{HEAD_PATH} changed after locked recovery: {error}"
            ))
        })?
        .ok_or_else(|| ExitError::conflict(format!("{HEAD_PATH} disappeared after recovery")))?;
    read.revalidate().map_err(|error| {
        ExitError::conflict(format!(
            "{HEAD_PATH} changed after locked recovery: {error}"
        ))
    })?;
    let raw = read.raw_bytes().to_vec();
    locked.exact_read = Some(read);
    Ok(Some(raw))
}

fn parse_learning_head(raw: &[u8], path: &Path) -> Result<LearningAnchorHead, ExitError> {
    let head: LearningAnchorHead = parse(raw, path)?;
    if head.schema_version != HEAD_SCHEMA {
        return Err(ExitError::invalid_value(
            "unsupported learning anchor head schema",
        ));
    }
    Ok(head)
}

fn load_head(locked: &mut LockedHeads) -> Result<LearningAnchorHead, ExitError> {
    let raw = snapshot_learning_head(locked)?.ok_or_else(|| {
        ExitError::invalid_value(
            "learning trust anchors are not provisioned; run trust-provision first",
        )
    })?;
    parse_learning_head(&raw, &locked.root.join(HEAD_PATH))
}

fn load_head_for_update(locked: &LockedHeads) -> Result<LearningAnchorHead, ExitError> {
    let raw = locked
        .reconciliation
        .as_ref()
        .and_then(|reconciliation| reconciliation.raw_bytes())
        .ok_or_else(|| {
            ExitError::invalid_value(
                "learning trust anchors are not provisioned; run trust-provision first",
            )
        })?;
    parse_learning_head(raw, &locked.root.join(HEAD_PATH))
}

fn persist_head(locked: LockedHeads, head: &LearningAnchorHead) -> Result<(), ExitError> {
    let bytes = yaml_serde::to_string(head)
        .map_err(|error| ExitError::failed(error.to_string()))?
        .into_bytes();
    if locked.exact_read.is_some() {
        return Err(ExitError::failed(
            "learning-head reconciliation authority was consumed as a read",
        ));
    }
    let reconciliation = locked
        .reconciliation
        .ok_or_else(|| ExitError::failed("learning-head reconciliation authority was consumed"))?;
    let mut installed = reconciliation
        .replace(&bytes)
        .map_err(|error| ExitError::failed(format!("cannot persist {HEAD_PATH}: {error}")))?;
    installed.revalidate().map_err(|error| {
        ExitError::conflict(format!(
            "{HEAD_PATH} selector changed while persistence completed: {error}"
        ))
    })
}

fn restore_reviewer_anchor(
    registry: DomainPackReviewerRegistryDocument,
    head: &ReviewerHead,
) -> Result<DomainPackReviewerRegistryAnchor, ExitError> {
    let value = &registry.domain_pack_reviewer_registry;
    if value.registry_id != head.registry_id
        || value.audience != head.audience
        || value.generation != head.generation
        || value.registry_digest != head.registry_digest
        || value.trust_policy_digest != head.trust_policy_digest
    {
        return Err(ExitError::conflict(
            "reviewer registry does not match operator-protected head",
        ));
    }
    DomainPackReviewerRegistryAnchor::from_operator_protected_head(
        registry,
        &head.trust_policy_digest,
        &head.full_digest,
    )
    .map_err(authority_error)
}

fn restore_reviewed_anchor(
    reviewer: &DomainPackReviewerRegistryAnchor,
    registry: DomainPackReviewedRegistryDocument,
    head: &ReviewedHead,
    now: u64,
) -> Result<ReviewedDomainPackRegistryAnchor, ExitError> {
    let value = &registry.domain_pack_reviewed_registry;
    if value.registry_id != head.registry_id
        || value.audience != head.audience
        || value.generation != head.generation
        || value.registry_digest != head.registry_digest
    {
        return Err(ExitError::conflict(
            "reviewed registry does not match operator-protected head",
        ));
    }
    ReviewedDomainPackRegistryAnchor::from_operator_protected_head(
        reviewer,
        registry,
        &head.registry_digest,
        now,
    )
    .map_err(authority_error)
}

fn trusted_now() -> Result<u64, ExitError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| ExitError::failed("system clock is before Unix epoch"))
}

fn read_many<T: DeserializeOwned>(paths: &[PathBuf]) -> Result<Vec<T>, ExitError> {
    paths.iter().map(|path| read_typed(path)).collect()
}
fn read_typed<T: DeserializeOwned>(path: &Path) -> Result<T, ExitError> {
    let raw = read_bounded(path, "typed learning document")?;
    parse(&raw, path)
}
fn parse<T: DeserializeOwned>(raw: &[u8], path: &Path) -> Result<T, ExitError> {
    let text = std::str::from_utf8(raw)
        .map_err(|error| ExitError::failed(format!("{} is not UTF-8: {error}", path.display())))?;
    yaml_serde::from_str(text).map_err(|error| {
        ExitError::failed(format!(
            "{} is not a closed typed document: {error}",
            path.display()
        ))
    })
}
fn read_bounded(path: &Path, label: &str) -> Result<Vec<u8>, ExitError> {
    let metadata = fs::metadata(path)
        .map_err(|error| ExitError::failed(format!("cannot inspect {label}: {error}")))?;
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(ExitError::invalid_value(format!(
            "{label} exceeds {MAX_DOCUMENT_BYTES} bytes"
        )));
    }
    fs::read(path).map_err(|error| ExitError::failed(format!("cannot read {label}: {error}")))
}
fn canonical_digest<T: Serialize>(value: &T) -> Result<String, ExitError> {
    let bytes = serde_json_canonicalizer::to_vec(value)
        .map_err(|error| ExitError::failed(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn authority_error(error: impl std::fmt::Display) -> ExitError {
    ExitError::invalid_value(format!("domain-pack learning authority rejected: {error}"))
}
fn store_error(error: impl std::fmt::Display) -> ExitError {
    ExitError::failed(format!("domain-pack learning store rejected: {error}"))
}
fn emit(command: &str, payload: serde_json::Value, json: bool) -> Result<(), ExitError> {
    crate::cli_util::emit_envelope(CliEnvelope::ok(command, payload), json)
}

fn usage() -> &'static str {
    "usage:\n  forge-core domain-pack learning capture --candidate-file <yaml> --state-root <.forge-method> [--json|--no-json]\n  forge-core domain-pack learning status --state-root <.forge-method> [--json|--no-json]\n  forge-core domain-pack learning evaluate --dossier-file <yaml> [--candidate-file <yaml>]... [--review-file <yaml>]... [--conflict-file <yaml>]... [--json|--no-json]\n  forge-core domain-pack learning conflict-check --dossier-file <yaml> [--candidate-file <yaml>]... [--review-file <yaml>]... [--conflict-file <yaml>]... [--json|--no-json]\n  forge-core domain-pack learning trust-provision --operator-root <dir> --reviewer-registry-file <yaml> --reviewed-registry-file <yaml> --project-root <dir> --state-root <.forge-method> --operator-acknowledge-trust-on-first-use I_UNDERSTAND_REVIEW_TRUST_ON_FIRST_USE [--json|--no-json]\n  forge-core domain-pack learning reviewer-rotate --operator-root <dir> --reviewer-registry-file <current-yaml> --proposed-reviewer-registry-file <yaml> --project-root <dir> --state-root <.forge-method> [--json|--no-json]\n  forge-core domain-pack learning registry-check --operator-root <dir> --reviewer-registry-file <yaml> --reviewed-registry-file <yaml> --project-root <dir> --state-root <.forge-method> [--json|--no-json]\n  forge-core domain-pack learning promote --operator-root <dir> --reviewer-registry-file <yaml> --reviewed-registry-file <current-yaml> --proposed-registry-file <yaml> --dossier-file <yaml> --candidate-file <yaml> [--candidate-file <yaml>]... [--conflict-file <yaml>]... --decision-file <yaml> --authorization-file <yaml> --review-file <yaml> --review-file <yaml> --project-root <dir> --state-root <.forge-method> [--json|--no-json]\n  caller-authored time is forbidden; trusted system time is used"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_anchor_head_cannot_decode_split_authority() {
        let reviewer = ReviewerHead {
            registry_id: StableId("reviewers.test".to_owned()),
            audience: "forge.test".to_owned(),
            generation: 0,
            registry_digest: "a".repeat(64),
            full_digest: "b".repeat(64),
            trust_policy_digest: "c".repeat(64),
        };
        let reviewed = ReviewedHead {
            registry_id: StableId("reviewed.test".to_owned()),
            audience: "forge.test".to_owned(),
            generation: 0,
            registry_digest: "d".repeat(64),
        };
        let combined = LearningAnchorHead {
            schema_version: HEAD_SCHEMA.to_owned(),
            reviewer,
            reviewed,
        };
        let encoded = yaml_serde::to_string(&combined).expect("combined anchor yaml");
        assert!(encoded.contains("reviewer:"));
        assert!(encoded.contains("reviewed:"));
        let split = encoded
            .lines()
            .take_while(|line| !line.starts_with("reviewed:"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            yaml_serde::from_str::<LearningAnchorHead>(&split).is_err(),
            "a reviewer-only persisted head must fail closed"
        );
    }
    #[test]
    fn retained_learning_head_fails_closed_across_parent_swap() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-learning-retained-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let expected = LearningAnchorHead {
            schema_version: HEAD_SCHEMA.to_owned(),
            reviewer: ReviewerHead {
                registry_id: StableId("reviewers.test".to_owned()),
                audience: "forge.test".to_owned(),
                generation: 1,
                registry_digest: "a".repeat(64),
                full_digest: "b".repeat(64),
                trust_policy_digest: "c".repeat(64),
            },
            reviewed: ReviewedHead {
                registry_id: StableId("reviewed.test".to_owned()),
                audience: "forge.test".to_owned(),
                generation: 2,
                registry_digest: "d".repeat(64),
            },
        };
        std::fs::write(
            root.join(HEAD_PATH),
            yaml_serde::to_string(&expected).unwrap(),
        )
        .unwrap();
        let mut locked = lock_heads(&root).unwrap();
        let moved = root.with_extension("retained");
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(HEAD_PATH), b"attacker: true\n").unwrap();

        assert!(load_head(&mut locked).is_err());
        drop(locked);
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(moved);
    }

    #[test]
    fn learning_head_persistence_rejects_byte_identical_session_substitution() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-learning-persist-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut next = LearningAnchorHead {
            schema_version: HEAD_SCHEMA.to_owned(),
            reviewer: ReviewerHead {
                registry_id: StableId("reviewers.test".to_owned()),
                audience: "forge.test".to_owned(),
                generation: 1,
                registry_digest: "a".repeat(64),
                full_digest: "b".repeat(64),
                trust_policy_digest: "c".repeat(64),
            },
            reviewed: ReviewedHead {
                registry_id: StableId("reviewed.test".to_owned()),
                audience: "forge.test".to_owned(),
                generation: 2,
                registry_digest: "d".repeat(64),
            },
        };
        let raw = yaml_serde::to_string(&next).unwrap();
        let target = root.join(HEAD_PATH);
        std::fs::write(&target, &raw).unwrap();
        let locked = lock_heads(&root).unwrap();
        let replacement = target.with_extension("replacement");
        std::fs::write(&replacement, &raw).unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::rename(&replacement, &target).unwrap();
        next.reviewed.generation += 1;

        assert!(persist_head(locked, &next).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), raw);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn learning_head_persistence_rejects_late_creation_after_recovered_absence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "forge-learning-absence-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let locked = lock_heads(&root).unwrap();
        std::fs::write(root.join(HEAD_PATH), b"attacker: true\n").unwrap();
        let head = LearningAnchorHead {
            schema_version: HEAD_SCHEMA.to_owned(),
            reviewer: ReviewerHead {
                registry_id: StableId("reviewers.test".to_owned()),
                audience: "forge.test".to_owned(),
                generation: 1,
                registry_digest: "a".repeat(64),
                full_digest: "b".repeat(64),
                trust_policy_digest: "c".repeat(64),
            },
            reviewed: ReviewedHead {
                registry_id: StableId("reviewed.test".to_owned()),
                audience: "forge.test".to_owned(),
                generation: 2,
                registry_digest: "d".repeat(64),
            },
        };

        assert!(persist_head(locked, &head).is_err());
        assert_eq!(
            std::fs::read(root.join(HEAD_PATH)).unwrap(),
            b"attacker: true\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
