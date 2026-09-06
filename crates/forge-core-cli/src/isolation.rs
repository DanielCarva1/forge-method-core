//! Governance CLI for layer-1 worktree isolation (S4.6).
//!
//! The engine DESCRIBES and VALIDATES isolation; it never runs git (DD31). This
//! module loads/persists isolation contracts to a directory (one YAML per
//! contract, mirroring the claims bus — DD22), enforces uniqueness via the
//! engine, and emits [`CliEnvelope`] payloads in the same DD9/DD10/DD17 shape as
//! the rest of the governance surface.
//!
//! `forge-core isolation propose` returns the contract AND a list of suggested
//! git commands the host agent runs to materialize the worktree. The merge-plan
//! command returns a deterministic ordered step list for merge-back.

use crate::claim::slug_for_file;
use crate::cli_error::ExitError;
use crate::cli_util::{
    emit_envelope_or_err, parse_strict_or_err, require_value_or_err, resolve_now_unix,
};
use crate::io_util::atomic_write;
use forge_core_command_surface::COMMAND_ISOLATION;
use forge_core_contracts::common::StableId;
use forge_core_contracts::isolation::{
    IsolationContract, IsolationContractDocument, IsolationError, IsolationStatus, MergePlan,
    MergePolicy,
};
use forge_core_contracts::{CliEnvelope, ExitReason, RepoPath, ENVELOPE_SCHEMA_VERSION};
use forge_core_decisions::isolation::{
    detect_isolation_conflict, propose_merge, transition_status, validate_isolation_claim_agent,
    validate_isolation_contract,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const LOCKFILE: &str = ".forge-isolation.lock";

#[allow(dead_code)]
const MAX_ISOLATION_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

#[allow(dead_code)]
/// Opaque retained authority for isolation-contract mutation.
pub(crate) struct LockedIsolationContracts {
    lock: crate::io_util::DirLock,
}

#[allow(dead_code)]
/// One exact isolation YAML and its stable identity projection.
pub(crate) struct IsolationEntrySnapshot {
    relative_path: String,
    raw: Vec<u8>,
    raw_sha256: String,
    contract: IsolationContract,
}

#[allow(dead_code)]
impl IsolationEntrySnapshot {
    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(crate) fn raw(&self) -> &[u8] {
        &self.raw
    }

    pub(crate) fn raw_sha256(&self) -> &str {
        &self.raw_sha256
    }

    pub(crate) const fn contract(&self) -> &IsolationContract {
        &self.contract
    }
}

#[allow(dead_code)]
/// Stable sorted no-follow projection captured under mutation authority.
pub(crate) struct IsolationContractsSnapshot {
    entries: Vec<IsolationEntrySnapshot>,
}

#[allow(dead_code)]
impl IsolationContractsSnapshot {
    pub(crate) fn entries(&self) -> &[IsolationEntrySnapshot] {
        &self.entries
    }
}

pub(crate) fn acquire_isolation_contracts_authority(
    isolation_dir: &Path,
) -> std::io::Result<LockedIsolationContracts> {
    let lock = crate::io_util::DirLock::acquire(isolation_dir, LOCKFILE)?;
    Ok(LockedIsolationContracts { lock })
}

#[allow(dead_code)]
pub(crate) fn snapshot_isolation_contracts_under_authority(
    locked: &LockedIsolationContracts,
) -> std::io::Result<IsolationContractsSnapshot> {
    use std::io::{Error, ErrorKind};
    let projected = locked
        .lock
        .directory_identity()
        .read_sorted_direct_files_bounded("yaml", MAX_ISOLATION_DOCUMENT_BYTES)?;
    let mut entries = Vec::with_capacity(projected.len());
    for (relative_file, raw) in projected {
        let document: IsolationContractDocument =
            yaml_serde::from_slice(&raw).map_err(|error| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("{}: {error}", relative_file.display()),
                )
            })?;
        let relative_path = relative_file
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "non-UTF-8 isolation path"))?
            .to_owned();
        entries.push(IsolationEntrySnapshot {
            relative_path,
            raw_sha256: format!("sha256:{:x}", Sha256::digest(&raw)),
            raw,
            contract: document.isolation_contract,
        });
    }
    Ok(IsolationContractsSnapshot { entries })
}

// ---------------------------------------------------------------------------
// payloads (DD17: machine-readable, same envelope as guide/* and claim/*)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct IsolationProposePayload {
    pub isolation: IsolationContract,
    pub contract_path: String,
    /// Literal `git ...` commands the host agent runs to materialize the
    /// worktree (DD31 — forge describes, the agent executes). Empty for a
    /// transition onto an existing contract.
    pub suggested_git_commands: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IsolationSummary {
    pub id: String,
    pub agent_id: String,
    pub branch_name: String,
    pub worktree_path: String,
    pub base_ref: String,
    pub status: String,
    pub merge_policy: String,
    pub claim_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IsolationStatusPayload {
    pub total: usize,
    pub active: Vec<IsolationSummary>,
}

// ---------------------------------------------------------------------------
// propose
// ---------------------------------------------------------------------------

/// Propose a new isolation contract: validate shape, detect collisions against
/// live contracts, write the YAML, and return the contract + suggested git
/// commands. Does NOT run git (DD31).
///
/// # Errors
/// - `RejectedByGate` (2) if the contract is malformed or collides with a live
///   contract — the typed [`IsolationError`] is carried in `error.code` so the
///   agent can self-correct.
/// - `EnvConfig` (5) if the isolation directory cannot be read/written.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn run_propose(
    isolation_dir: &Path,
    claims_dir: Option<&Path>,
    agent_id: &StableId,
    branch_name: &str,
    worktree_path: &str,
    base_ref: &str,
    merge_policy: MergePolicy,
    claim_id: Option<StableId>,
    isolation_id: &str,
    now_unix: i64,
) -> CliEnvelope<IsolationProposePayload> {
    let contract = IsolationContract {
        id: StableId(isolation_id.to_string()),
        agent_id: agent_id.clone(),
        branch_name: branch_name.to_string(),
        worktree_path: RepoPath(worktree_path.to_string()),
        base_ref: base_ref.to_string(),
        created_at: forge_core_decisions::unix_to_rfc3339(now_unix),
        status: IsolationStatus::Proposed,
        merge_policy,
        claim_id,
    };

    // 1) intrinsic shape
    if let Err(e) = validate_isolation_contract(&contract) {
        return rejection("propose", e, &contract.id);
    }

    // 2) Resolve and retain the linked claim authority before taking the
    // isolation lock. This matches the repository-wide claim -> isolation lock
    // order and prevents the claim owner changing between validation and save.
    let _claim_lock = if let Some(claim_id) = contract.claim_id.as_ref() {
        let Some(claims_dir) = claims_dir else {
            return CliEnvelope::err(
                "isolation propose",
                ExitReason::EnvConfig,
                format!(
                    "linked claim '{}' requires an authoritative claims directory",
                    claim_id.0
                ),
            );
        };
        let lock = match crate::claim::acquire_claim_cache_authority(claims_dir) {
            Ok(lock) => lock,
            Err(error) => {
                return CliEnvelope::err(
                    "isolation propose",
                    ExitReason::EnvConfig,
                    format!(
                        "cannot lock authoritative claims directory {}: {error}",
                        claims_dir.display()
                    ),
                );
            }
        };
        let (claims, errors) = crate::claim::load_claims(claims_dir);
        if !errors.is_empty() {
            return CliEnvelope::err(
                "isolation propose",
                ExitReason::EnvConfig,
                format!(
                    "authoritative claims state has {} error(s): {}",
                    errors.len(),
                    errors.join("; ")
                ),
            );
        }
        let Some(claim) = claims.iter().find(|claim| claim.id.0 == claim_id.0) else {
            return CliEnvelope::err(
                "isolation propose",
                ExitReason::InvalidDecisionShape,
                format!(
                    "linked claim '{}' was not found in authoritative claim state",
                    claim_id.0
                ),
            );
        };
        if let Err(error) =
            validate_isolation_claim_agent(&contract, &claim.claim.claimant_agent_id)
        {
            return rejection("propose", error, &contract.id);
        }
        Some(lock)
    } else {
        None
    };

    // 3) collision against existing live contracts (under lock)
    let lock = match acquire_isolation_contracts_authority(isolation_dir) {
        Ok(l) => l,
        Err(e) => return env_config("propose", isolation_dir, &e.to_string()),
    };
    let _ = lock;
    let (existing, errs) = load_isolations(isolation_dir);
    if let Some(env) = env_config_if_errors("propose", isolation_dir, &errs) {
        return env;
    }
    let refs: Vec<&IsolationContract> = existing.iter().collect();
    if let Err(e) = detect_isolation_conflict(&contract, &refs) {
        return rejection("propose", e, &contract.id);
    }
    // 4) persist
    let path = match save_isolation(isolation_dir, &contract) {
        Ok(p) => p,
        Err(e) => return env_config("propose", isolation_dir, &e.to_string()),
    };

    let suggested = suggested_worktree_commands(&contract);
    CliEnvelope::ok(
        "isolation propose",
        IsolationProposePayload {
            isolation: contract,
            contract_path: path.display().to_string(),
            suggested_git_commands: suggested,
        },
    )
}

/// Attach an explicitly selected live claim without reproposing an isolation.
/// Claim authority precedes isolation authority, matching `run_propose`.
fn run_link_claim(
    isolation_dir: &Path,
    claims_dir: &Path,
    isolation_id: &StableId,
    claim_id: &StableId,
    now_unix: Option<i64>,
) -> CliEnvelope<IsolationProposePayload> {
    let _claim_lock = match crate::claim::acquire_claim_cache_authority(claims_dir) {
        Ok(lock) => lock,
        Err(error) => return env_config("link-claim", claims_dir, &error.to_string()),
    };
    let isolation_lock = match acquire_isolation_contracts_authority(isolation_dir) {
        Ok(lock) => lock,
        Err(error) => return env_config("link-claim", isolation_dir, &error.to_string()),
    };
    let snapshot = match snapshot_isolation_contracts_under_authority(&isolation_lock) {
        Ok(snapshot) => snapshot,
        Err(error) => return env_config("link-claim", isolation_dir, &error.to_string()),
    };
    let mut matches = snapshot
        .entries
        .iter()
        .filter(|entry| &entry.contract.id == isolation_id);
    let Some(entry) = matches.next() else {
        return CliEnvelope::err(
            "isolation link-claim",
            ExitReason::InvalidDecisionShape,
            format!("isolation '{}' not found", isolation_id.0),
        );
    };
    let relative_path = format!("{}.yaml", slug_for_file(&isolation_id.0));
    if matches.next().is_some() || entry.relative_path != relative_path {
        return env_config(
            "link-claim",
            isolation_dir,
            "isolation id is duplicated or stored under a noncanonical filename",
        );
    }
    let mut contract = entry.contract.clone();
    if !matches!(
        contract.status,
        IsolationStatus::Proposed | IsolationStatus::Active
    ) {
        return CliEnvelope::err(
            "isolation link-claim",
            ExitReason::RejectedByGate,
            "claim attachment requires a proposed or active isolation",
        );
    }
    if contract
        .claim_id
        .as_ref()
        .is_some_and(|linked| linked != claim_id)
    {
        return CliEnvelope::err(
            "isolation link-claim",
            ExitReason::RejectedByGate,
            "isolation is already linked to a different claim",
        );
    }
    let (claims, errors) = crate::claim::load_claims(claims_dir);
    if !errors.is_empty() {
        return env_config("link-claim", claims_dir, &errors.join("; "));
    }
    let Some(claim) = claims.iter().find(|claim| claim.id.0 == claim_id.0) else {
        return CliEnvelope::err(
            "isolation link-claim",
            ExitReason::InvalidDecisionShape,
            format!(
                "linked claim '{}' was not found in authoritative claim state",
                claim_id.0
            ),
        );
    };
    // Resolve wall-clock time only after both locks and authoritative reads.
    if !forge_core_decisions::is_live(claim, resolve_now_unix(now_unix)) {
        return CliEnvelope::err(
            "isolation link-claim",
            ExitReason::RejectedByGate,
            "linked claim is not live or its lease has expired",
        );
    }
    let already_linked = contract.claim_id.is_some();
    contract.claim_id = Some(claim_id.clone());
    if let Err(error) = validate_isolation_claim_agent(&contract, &claim.claim.claimant_agent_id) {
        return rejection("link-claim", error, isolation_id);
    }
    let path = if already_linked {
        isolation_dir.join(relative_path)
    } else {
        match save_isolation(isolation_dir, &contract) {
            Ok(path) => path,
            Err(error) => return env_config("link-claim", isolation_dir, &error.to_string()),
        }
    };
    CliEnvelope::ok(
        "isolation link-claim",
        IsolationProposePayload {
            isolation: contract,
            contract_path: path.display().to_string(),
            suggested_git_commands: Vec::new(),
        },
    )
}

/// Build the literal `git ...` commands an agent runs to create the worktree
// and branch described by `c`. Every interpolated value is POSIX single-quote
// escaped (review S4.6 C1: a stray `;`/`$()`/backtick in branch_name,
// worktree_path, or base_ref must never yield a second shell command when an
// agent copy-pastes the suggestion).
#[must_use]
pub fn suggested_worktree_commands(c: &IsolationContract) -> Vec<String> {
    vec![
        format!(
            "git worktree add -b {} {} {}",
            shell_quote(&c.branch_name),
            shell_quote(&c.worktree_path.0),
            shell_quote(&c.base_ref)
        ),
        format!("cd {}", shell_quote(&c.worktree_path.0)),
    ]
}

/// POSIX single-quote a string so it is safe to interpolate into a shell
/// command an agent will copy-paste. Wraps in `'...'` and escapes any embedded
/// single quote as `'\''` (the standard close-quote/escaped-quote/reopen).
/// This is the ONLY correct way to put an untrusted value into a shell
/// command string.
#[must_use]
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

/// List isolation contracts. Optionally filter by agent. Read-only (no lock).
#[must_use]
pub fn run_status(
    isolation_dir: &Path,
    agent_filter: Option<&StableId>,
) -> CliEnvelope<IsolationStatusPayload> {
    let (contracts, errs) = load_isolations(isolation_dir);
    if let Some(env) = env_config_if_errors("status", isolation_dir, &errs) {
        return env;
    }
    let active: Vec<IsolationSummary> = contracts
        .into_iter()
        .filter(|c| agent_filter.is_none_or(|a| &c.agent_id == a))
        .map(summary_of)
        .collect();
    let total = active.len();
    CliEnvelope::ok("isolation status", IsolationStatusPayload { total, active })
}

// ---------------------------------------------------------------------------
// merge-plan
// ---------------------------------------------------------------------------

/// Emit a deterministic merge plan for an isolation contract. Read-only.
///
/// # Errors
/// - `InvalidDecisionShape` (3) if the id is not found.
#[must_use]
pub fn run_merge_plan(
    isolation_dir: &Path,
    isolation_id: &StableId,
    now_unix: i64,
) -> CliEnvelope<MergePlan> {
    let (contracts, errs) = load_isolations(isolation_dir);
    if let Some(env) = env_config_if_errors("merge-plan", isolation_dir, &errs) {
        return env;
    }
    let Some(contract) = contracts.into_iter().find(|c| &c.id == isolation_id) else {
        return CliEnvelope::err(
            "isolation merge-plan",
            ExitReason::InvalidDecisionShape,
            format!("isolation '{}' not found", isolation_id.0),
        );
    };
    let plan = propose_merge(&contract, now_unix);
    CliEnvelope::ok("isolation merge-plan", plan)
}

// ---------------------------------------------------------------------------
// transition (state machine)
// ---------------------------------------------------------------------------

/// Apply a lifecycle transition to an isolation contract (DD35). Load under
/// lock, validate the transition, rewrite the contract atomically.
///
/// # Errors
/// - `InvalidDecisionShape` (3) if the id is not found.
/// - `RejectedByGate` (2) if the transition is illegal.
/// - `EnvConfig` (5) on IO errors.
#[must_use]
pub fn run_transition(
    isolation_dir: &Path,
    isolation_id: &StableId,
    to: IsolationStatus,
    now_unix: i64,
) -> CliEnvelope<IsolationProposePayload> {
    let _ = now_unix;
    let lock = match acquire_isolation_contracts_authority(isolation_dir) {
        Ok(l) => l,
        Err(e) => return env_config("transition", isolation_dir, &e.to_string()),
    };
    let _ = lock;
    let (mut contracts, errs) = load_isolations(isolation_dir);
    if let Some(env) = env_config_if_errors("transition", isolation_dir, &errs) {
        return env;
    }
    let Some(idx) = contracts.iter().position(|c| &c.id == isolation_id) else {
        return CliEnvelope::err(
            "isolation transition",
            ExitReason::InvalidDecisionShape,
            format!("isolation '{}' not found", isolation_id.0),
        );
    };
    let from = contracts[idx].status;
    match transition_status(from, to) {
        Ok(new) => {
            contracts[idx].status = new;
            let path = match save_isolation(isolation_dir, &contracts[idx]) {
                Ok(p) => p,
                Err(e) => return env_config("transition", isolation_dir, &e.to_string()),
            };
            let suggested = if new == IsolationStatus::Merging {
                merge_back_commands(&contracts[idx])
            } else {
                Vec::new()
            };
            CliEnvelope::ok(
                "isolation transition",
                IsolationProposePayload {
                    isolation: contracts[idx].clone(),
                    contract_path: path.display().to_string(),
                    suggested_git_commands: suggested,
                },
            )
        }
        Err(e) => rejection("transition", e, &contracts[idx].id),
    }
}

/// Literal `git ...` commands for the merge-back, derived from a fresh merge
/// plan (so they always match the contract's `merge_policy`).
fn merge_back_commands(c: &IsolationContract) -> Vec<String> {
    let plan = propose_merge(c, 0);
    plan.steps
        .into_iter()
        .map(|s| {
            let argv = s
                .args
                .iter()
                .map(|a| shell_quote(a))
                .collect::<Vec<_>>()
                .join(" ");
            format!("git {} {argv}", action_subcommand(s.action))
        })
        .collect()
}

fn action_subcommand(a: forge_core_contracts::isolation::GitAction) -> &'static str {
    use forge_core_contracts::isolation::GitAction::{
        BranchDelete, Checkout, Fetch, Merge, Rebase, Squash, WorktreeAdd, WorktreeRemove,
    };
    match a {
        WorktreeAdd | WorktreeRemove => "worktree",
        Checkout => "checkout",
        Fetch => "fetch",
        Rebase => "rebase",
        Merge => "merge",
        Squash => "merge --squash",
        BranchDelete => "branch",
    }
}

// ---------------------------------------------------------------------------
// IO helpers (mirror claim.rs load/save pattern; DD22 one-YAML-per-contract)
// ---------------------------------------------------------------------------

/// Load every `*.yaml` isolation document in `dir`. Malformed files surface as
/// errors (never silently dropped — would corrupt the coordination picture).
#[must_use]
pub fn load_isolations(dir: &Path) -> (Vec<IsolationContract>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (out, errors),
        Err(e) => {
            errors.push(format!("{}: cannot read isolation dir: {e}", dir.display()));
            return (out, errors);
        }
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        match entry {
            Ok(e) => paths.push(e.path()),
            Err(e) => errors.push(format!("{}: dir entry error: {e}", dir.display())),
        }
    }
    paths.retain(|p| p.extension().is_some_and(|x| x == "yaml"));
    paths.sort();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            errors.push(format!("{}: unreadable", path.display()));
            continue;
        };
        match yaml_serde::from_str::<IsolationContractDocument>(&text) {
            Ok(doc) => out.push(doc.isolation_contract),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    (out, errors)
}

/// Persist an isolation contract atomically (temp + rename). Filename is the
/// slugified id (no traversal — [`slug_for_file`]).
/// Persist an [`IsolationContract`] as a YAML envelope inside `dir`.
///
/// The file is named after the contract's slugified id and written
/// atomically via [`atomic_write`].
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when `dir` cannot be created,
/// when YAML serialization fails (surfaced as `InvalidData`), or when the
/// atomic write fails.
pub fn save_isolation(dir: &Path, c: &IsolationContract) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let doc = IsolationContractDocument {
        schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
        isolation_contract: c.clone(),
    };
    let yaml = yaml_serde::to_string(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = dir.join(format!("{}.yaml", slug_for_file(&c.id.0)));
    atomic_write(&path, &yaml)?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// small envelope builders
// ---------------------------------------------------------------------------

fn rejection(
    command: &str,
    e: IsolationError,
    id: &StableId,
) -> CliEnvelope<IsolationProposePayload> {
    // Stable snake_case code derived from the variant, independent of Debug.
    let code = match &e {
        IsolationError::IllegalBranchName { .. } => "illegal_branch_name",
        IsolationError::IllegalWorktreePath { .. } => "illegal_worktree_path",
        IsolationError::DuplicateBranch { .. } => "duplicate_branch",
        IsolationError::DuplicateWorktreePath { .. } => "duplicate_worktree_path",
        IsolationError::ClaimAgentMismatch { .. } => "claim_agent_mismatch",
        IsolationError::UnparseableCreatedAt { .. } => "unparseable_created_at",
        IsolationError::EmptyBaseRef => "empty_base_ref",
        IsolationError::ShellMetacharInField { .. } => "shell_metachar_in_field",
        IsolationError::EmptyAgentId => "empty_agent_id",
        IsolationError::IllegalTransition { .. } => "illegal_transition",
    };
    CliEnvelope::err(
        command,
        ExitReason::RejectedByGate,
        format!("isolation '{}' rejected: {e} (code: {code})", id.0),
    )
}

fn env_config<T: serde::Serialize>(command: &str, dir: &Path, detail: &str) -> CliEnvelope<T> {
    CliEnvelope::err(
        command,
        ExitReason::EnvConfig,
        format!("{}: isolation dir {}: {}", command, dir.display(), detail),
    )
}

fn env_config_if_errors<T: serde::Serialize>(
    command: &str,
    _dir: &Path,
    errors: &[String],
) -> Option<CliEnvelope<T>> {
    if errors.is_empty() {
        None
    } else {
        Some(CliEnvelope::err(
            command,
            ExitReason::EnvConfig,
            format!(
                "isolation dir has {} malformed file(s): {}",
                errors.len(),
                errors.join("; ")
            ),
        ))
    }
}

fn summary_of(c: IsolationContract) -> IsolationSummary {
    IsolationSummary {
        id: c.id.0,
        agent_id: c.agent_id.0,
        branch_name: c.branch_name,
        worktree_path: c.worktree_path.0,
        base_ref: c.base_ref,
        status: status_str(c.status).to_string(),
        merge_policy: policy_str(c.merge_policy).to_string(),
        claim_id: c.claim_id.map(|s| s.0),
        created_at: c.created_at,
    }
}

#[must_use]
pub fn status_str(s: IsolationStatus) -> &'static str {
    use IsolationStatus::{Abandoned, Active, Merged, Merging, Proposed};
    match s {
        Proposed => "proposed",
        Active => "active",
        Merging => "merging",
        Merged => "merged",
        Abandoned => "abandoned",
    }
}

#[must_use]
pub fn policy_str(p: MergePolicy) -> &'static str {
    match p {
        MergePolicy::Rebase => "rebase",
        MergePolicy::Merge => "merge",
        MergePolicy::Squash => "squash",
    }
}

/// Hand-rolled error enum for [`parse_merge_policy`] (no `thiserror`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergePolicyParseError {
    /// The input did not match any of the known merge-policy values.
    Unknown { raw: String },
}

impl std::fmt::Display for MergePolicyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown { raw } => {
                write!(
                    f,
                    "unknown merge-policy '{raw}' (expected: rebase | merge | squash)"
                )
            }
        }
    }
}

/// Hand-rolled error enum for [`parse_status`] (no `thiserror`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IsolationStatusParseError {
    /// The input did not match any of the known isolation-status values.
    Unknown { raw: String },
}

impl std::fmt::Display for IsolationStatusParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown { raw } => write!(
                f,
                "unknown status '{raw}' (expected: proposed|active|merging|merged|abandoned)"
            ),
        }
    }
}

/// Parse a CLI string into a [`MergePolicy`]. Exits 3 on unknown value
/// (consistent with DD10 — invalid input shape, not env error).
/// Parse a CLI string into a [`MergePolicy`].
///
/// # Errors
///
/// Returns [`MergePolicyParseError::Unknown`] when `raw` is not one of the
/// recognised aliases (`rebase`, `merge`, `squash`).
pub fn parse_merge_policy(raw: &str) -> Result<MergePolicy, MergePolicyParseError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "rebase" => Ok(MergePolicy::Rebase),
        "merge" => Ok(MergePolicy::Merge),
        "squash" => Ok(MergePolicy::Squash),
        other => Err(MergePolicyParseError::Unknown {
            raw: other.to_string(),
        }),
    }
}

/// Parse a CLI string into an [`IsolationStatus`].
///
/// # Errors
///
/// Returns [`IsolationStatusParseError::Unknown`] when `raw` is not one of
/// the recognised aliases (`proposed`, `active`, `merging`, `merged`,
/// `abandoned`).
pub fn parse_status(raw: &str) -> Result<IsolationStatus, IsolationStatusParseError> {
    use IsolationStatus::{Abandoned, Active, Merged, Merging, Proposed};
    match raw.trim().to_ascii_lowercase().as_str() {
        "proposed" => Ok(Proposed),
        "active" => Ok(Active),
        "merging" => Ok(Merging),
        "merged" => Ok(Merged),
        "abandoned" => Ok(Abandoned),
        other => Err(IsolationStatusParseError::Unknown {
            raw: other.to_string(),
        }),
    }
}
/// Dispatch entrypoint for the `forge-core isolation` subcommand tree.
///
/// Routes to `propose`, `status`, `merge-plan`, or `transition` based on
/// `args[1]`, and prints usage on `--help` / unknown subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` when the subcommand is unknown. Sub-command
/// dispatchers may surface their own `ExitError::usage` or `ExitError::failed`
/// variants for missing arguments or command failures.
pub fn run_isolation_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "propose" => run_isolation_propose(&args[2..]),
        "link-claim" => run_isolation_link_claim(&args[2..]),
        "status" => run_isolation_status(&args[2..]),
        "merge-plan" => run_isolation_merge_plan(&args[2..]),
        "transition" => run_isolation_transition(&args[2..]),
        "--help" | "-h" | "help" => {
            print_isolation_usage();
            println!("  Defaults: without --isolation-dir, resolves --root as a Forge project and uses <state_root>/contracts/isolations; --isolation-dir is an explicit override.");
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core isolation: unknown subcommand '{other}'. Try: {hint}",
            hint = isolation_subcommand_hint()
        ))),
    }
}

fn print_isolation_usage() {
    println!("forge-core isolation <subcommand> [options]");
    for line in COMMAND_ISOLATION.local_usage_lines() {
        println!("  {line}");
    }
}

fn isolation_subcommand_hint() -> String {
    COMMAND_ISOLATION.concrete_subcommand_hint()
}

fn isolation_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_ISOLATION
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core isolation <subcommand> [options]")
}

#[must_use]
fn unknown_isolation_arg(subcommand: &str, arg: &str) -> ExitError {
    eprintln!("isolation {subcommand}: unrecognized argument '{arg}'");
    ExitError::invalid_value(format!(
        "isolation {subcommand}: unrecognized argument '{arg}'"
    ))
}

#[must_use]
/// Resolves `--isolation-dir` to a [`PathBuf`], defaulting to
/// `<state_root>/contracts/isolations` resolved from `--root`.
///
/// # Errors
///
/// Returns `ExitError::env_config` (via [`emit_envelope_or_err`]) when
/// `--isolation-dir` is unset and project resolution fails or the resolved
/// `state_root` does not exist / is not a directory.
#[allow(clippy::double_must_use)]
pub fn resolve_isolation_dir_or_err(
    command: &str,
    isolation_dir: Option<PathBuf>,
    root: &std::path::Path,
    want_json: bool,
) -> Result<PathBuf, ExitError> {
    if let Some(isolation_dir) = isolation_dir {
        return Ok(isolation_dir);
    }

    match crate::project_cmd::resolve_project(root) {
        Ok(project) if project.state_exists => {
            let state_root = PathBuf::from(project.state_root);
            if state_root.is_dir() {
                Ok(state_root.join("contracts").join("isolations"))
            } else {
                let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                    command,
                    forge_core_contracts::ExitReason::EnvConfig,
                    format!(
                        "resolved Forge state_root is not a directory for isolation command: {}; fix {} or recreate the sidecar .forge-method directory",
                        state_root.display(),
                        forge_core_contracts::PROJECT_LINK_FILE_NAME
                    ),
                );
                // Print the envelope to mirror legacy behavior, then surface the
                // envelope's exit code as an ExitError.
                crate::cli_util::emit_envelope_or_err("isolation", env, want_json)
                    .map(|()| unreachable!("emit_envelope_or_err Ok path is unreachable: envelope always non-zero here"))
            }
        }
        Ok(project) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                forge_core_contracts::ExitReason::EnvConfig,
                format!(
                    "resolved Forge state_root does not exist for isolation command: {}; create the sidecar .forge-method directory or fix {}",
                    project.state_root,
                    forge_core_contracts::PROJECT_LINK_FILE_NAME
                ),
            );
            crate::cli_util::emit_envelope_or_err("isolation", env, want_json).map(|()| {
                unreachable!(
                    "emit_envelope_or_err Ok path is unreachable: envelope always non-zero here"
                )
            })
        }
        Err(err) => {
            let env = forge_core_contracts::CliEnvelope::<serde_json::Value>::err(
                command,
                err.exit_reason(),
                format!("project resolve failed for isolation command: {err}"),
            );
            crate::cli_util::emit_envelope_or_err("isolation", env, want_json).map(|()| {
                unreachable!(
                    "emit_envelope_or_err Ok path is unreachable: envelope always non-zero here"
                )
            })
        }
    }
}

/// Runs the `forge-core isolation propose` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when required flags
/// (`--agent`, `--branch`, `--worktree-path`) are missing or `--merge-policy`
/// carries an unknown alias, and `ExitError::with_code` (via
/// [`emit_envelope_or_err`]) when the propose operation surfaces a non-zero
/// exit code.
pub fn run_isolation_propose(args: &[String]) -> Result<(), ExitError> {
    use crate::claim::slug_for_file;
    use forge_core_contracts::isolation::MergePolicy;
    use forge_core_contracts::StableId;

    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut agent = String::new();
    let mut branch = String::new();
    let mut worktree_path = String::new();
    let mut base_ref = String::from("main");
    let mut merge_policy = MergePolicy::Rebase;
    let mut claim_id: Option<String> = None;
    let mut isolation_id: Option<String> = None;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--agent" => {
                idx += 1;
                agent = require_value_or_err(args, idx, "agent")?;
            }
            "--branch" => {
                idx += 1;
                branch = require_value_or_err(args, idx, "branch")?;
            }
            "--worktree-path" => {
                idx += 1;
                worktree_path = require_value_or_err(args, idx, "worktree-path")?;
            }
            "--base-ref" => {
                idx += 1;
                base_ref = require_value_or_err(args, idx, "base-ref")?;
            }
            "--id" => {
                idx += 1;
                isolation_id = Some(require_value_or_err(args, idx, "id")?);
            }
            "--merge-policy" => {
                idx += 1;
                merge_policy =
                    match parse_merge_policy(&require_value_or_err(args, idx, "merge-policy")?) {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("isolation propose: {e}");
                            return Err(ExitError::invalid_value(format!(
                                "isolation propose: {e}"
                            )));
                        }
                    };
            }
            "--claim" => {
                idx += 1;
                claim_id = Some(require_value_or_err(args, idx, "claim")?);
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "isolation-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", isolation_command_surface_usage_line_for("propose"));
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            other => return Err(unknown_isolation_arg("propose", other)),
        }
        idx += 1;
    }
    if agent.is_empty() || branch.is_empty() || worktree_path.is_empty() {
        eprintln!("isolation propose: --agent, --branch, --worktree-path are all required");
        return Err(ExitError::invalid_value(
            "isolation propose: --agent, --branch, --worktree-path are all required",
        ));
    }
    let now = resolve_now_unix(now_unix);
    let id = isolation_id.unwrap_or_else(|| format!("iso-{}-{}", slug_for_file(&branch), now));
    let isolation_dir =
        resolve_isolation_dir_or_err("isolation.propose", isolation_dir, &root, want_json)?;
    let claims_dir = if claim_id.is_some() {
        Some(crate::claim::resolve_claims_dir_or_err(
            "isolation.propose",
            None,
            &root,
            want_json,
        )?)
    } else {
        None
    };
    let env = run_propose(
        &isolation_dir,
        claims_dir.as_deref(),
        &StableId(agent),
        &branch,
        &worktree_path,
        &base_ref,
        merge_policy,
        claim_id.map(StableId),
        &id,
        now,
    );
    emit_envelope_or_err("isolation", env, want_json)
}

/// Runs the `forge-core isolation status` subcommand.
///
/// # Errors
///
/// Returns `ExitError::with_code` (via [`emit_envelope_or_err`]) when the
/// status read surfaces a non-zero exit code, and `ExitError::env_config`
/// (via [`resolve_isolation_dir_or_err`]) when project resolution fails.
pub fn run_isolation_status(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::StableId;
    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut want_json = true;
    let mut agent: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--agent" => {
                idx += 1;
                agent = Some(require_value_or_err(args, idx, "agent")?);
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "isolation-dir",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", isolation_command_surface_usage_line_for("status"));
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            other => return Err(unknown_isolation_arg("status", other)),
        }
        idx += 1;
    }
    let isolation_dir =
        resolve_isolation_dir_or_err("isolation.status", isolation_dir, &root, want_json)?;
    let env = run_status(
        &isolation_dir,
        agent.as_ref().map(|a| StableId(a.clone())).as_ref(),
    );
    emit_envelope_or_err("isolation", env, want_json)
}

/// Runs the `forge-core isolation merge-plan` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `--id` is missing, and
/// `ExitError::with_code` (via [`emit_envelope_or_err`]) when the
/// merge-plan operation surfaces a non-zero exit code.
pub fn run_isolation_merge_plan(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::StableId;
    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut id = String::new();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--id" => {
                idx += 1;
                id = require_value_or_err(args, idx, "id")?;
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "isolation-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", isolation_command_surface_usage_line_for("merge-plan"));
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            other => return Err(unknown_isolation_arg("merge-plan", other)),
        }
        idx += 1;
    }
    if id.is_empty() {
        eprintln!("isolation merge-plan: --id <isolation-id> is required");
        return Err(ExitError::invalid_value(
            "isolation merge-plan: --id <isolation-id> is required",
        ));
    }
    let isolation_dir =
        resolve_isolation_dir_or_err("isolation.merge-plan", isolation_dir, &root, want_json)?;
    let env = run_merge_plan(&isolation_dir, &StableId(id), resolve_now_unix(now_unix));
    emit_envelope_or_err("isolation", env, want_json)
}

/// Runs the `forge-core isolation transition` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `--id` or `--to` is missing or
/// `--to` is not a recognised status alias, and `ExitError::with_code`
/// (via [`emit_envelope_or_err`]) when the transition surfaces a non-zero
/// exit code.
pub fn run_isolation_transition(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::StableId;
    let mut isolation_dir: Option<PathBuf> = None;
    let mut root = PathBuf::from(".");
    let mut now_unix: Option<i64> = None;
    let mut want_json = true;
    let mut id = String::new();
    let mut to_raw = String::new();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--id" => {
                idx += 1;
                id = require_value_or_err(args, idx, "id")?;
            }
            "--to" => {
                idx += 1;
                to_raw = require_value_or_err(args, idx, "to")?;
            }
            "--isolation-dir" => {
                idx += 1;
                isolation_dir = Some(PathBuf::from(require_value_or_err(
                    args,
                    idx,
                    "isolation-dir",
                )?));
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", isolation_command_surface_usage_line_for("transition"));
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            other => return Err(unknown_isolation_arg("transition", other)),
        }
        idx += 1;
    }
    if id.is_empty() || to_raw.is_empty() {
        eprintln!("isolation transition: --id and --to are both required");
        return Err(ExitError::invalid_value(
            "isolation transition: --id and --to are both required",
        ));
    }
    let to = match parse_status(&to_raw) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("isolation transition: {e}");
            return Err(ExitError::invalid_value(format!(
                "isolation transition: {e}"
            )));
        }
    };
    let isolation_dir =
        resolve_isolation_dir_or_err("isolation.transition", isolation_dir, &root, want_json)?;
    let env = run_transition(
        &isolation_dir,
        &StableId(id),
        to,
        resolve_now_unix(now_unix),
    );
    emit_envelope_or_err("isolation", env, want_json)
}

fn run_isolation_link_claim(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut id = String::new();
    let mut claim_id = String::new();
    let mut now_unix = None;
    let mut want_json = true;
    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--id" => {
                idx += 1;
                id = require_value_or_err(args, idx, "id")?;
            }
            "--claim" => {
                idx += 1;
                claim_id = require_value_or_err(args, idx, "claim")?;
            }
            "--now-unix" => {
                idx += 1;
                now_unix = Some(parse_strict_or_err(
                    &require_value_or_err(args, idx, "now-unix")?,
                    "now-unix",
                )?);
            }
            "--json" => want_json = true,
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("{}", isolation_command_surface_usage_line_for("link-claim"));
                return Ok(());
            }
            other => return Err(unknown_isolation_arg("link-claim", other)),
        }
        idx += 1;
    }
    if id.is_empty() || claim_id.is_empty() {
        return Err(ExitError::invalid_value(
            "isolation link-claim: --id and --claim are both required",
        ));
    }
    let isolation_dir =
        resolve_isolation_dir_or_err("isolation.link-claim", None, &root, want_json)?;
    let claims_dir =
        crate::claim::resolve_claims_dir_or_err("isolation.link-claim", None, &root, want_json)?;
    emit_envelope_or_err(
        "isolation",
        run_link_claim(
            &isolation_dir,
            &claims_dir,
            &StableId(id),
            &StableId(claim_id),
            now_unix,
        ),
        want_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("iso-test-{}-{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    const NOW: i64 = 1_800_000_000;

    fn propose_ok(
        d: &Path,
        agent: &str,
        branch: &str,
        path: &str,
    ) -> CliEnvelope<IsolationProposePayload> {
        run_propose(
            d,
            None,
            &StableId(agent.into()),
            branch,
            path,
            "main",
            MergePolicy::Rebase,
            None,
            &format!("iso-{agent}-{}", slug_for_file(branch)),
            NOW,
        )
    }

    fn link_claim_fixture() -> (PathBuf, PathBuf, String) {
        let parent = dir();
        let root = parent.join("app");
        let state = parent.join("sidecar/.forge-method");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&state).unwrap();
        fs::write(root.join(forge_core_contracts::PROJECT_LINK_FILE_NAME),
            "schema_version: forge_project_link_v1\nproject_id: app\nsidecar_root: ../sidecar\nstate_root: ../sidecar/.forge-method\n").unwrap();
        crate::claim::run_claim_acquire(&args(&[
            "--root",
            root.to_str().unwrap(),
            "--scope",
            "product-area",
            "--id",
            "link-test",
            "--agent",
            "alice",
            "--path",
            "src/a.rs",
            "--now-unix",
            &NOW.to_string(),
        ]))
        .unwrap();
        let (claims, errors) = crate::claim::load_claims(&state.join("claims-active"));
        assert!(errors.is_empty());
        let claim_id = claims[0].id.0.clone();
        let envelope = propose_ok(
            &state.join("contracts/isolations"),
            "alice",
            "alice/link",
            "../.forge-worktrees/alice/link",
        );
        assert!(envelope.ok);
        (root, state, claim_id)
    }

    #[test]
    fn isolation_link_claim_public_command_preserves_contract_and_retry_bytes() {
        let (root, state, claim_id) = link_claim_fixture();
        let isolation_dir = state.join("contracts/isolations");
        let mut expected = load_isolations(&isolation_dir).0.remove(0);
        let command = args(&[
            "isolation",
            "link-claim",
            "--root",
            root.to_str().unwrap(),
            "--id",
            &expected.id.0,
            "--claim",
            &claim_id,
            "--now-unix",
            &NOW.to_string(),
        ]);
        assert!(
            run_isolation_command(&command).is_ok(),
            "public link-claim must attach a live owned claim"
        );
        expected.claim_id = Some(StableId(claim_id));
        assert_eq!(load_isolations(&isolation_dir).0, vec![expected.clone()]);
        let path = isolation_dir.join(format!("{}.yaml", slug_for_file(&expected.id.0)));
        let annotated = format!(
            "{}\n# preserve exact retry bytes\n",
            fs::read_to_string(&path).unwrap()
        );
        fs::write(&path, annotated).unwrap();
        let before = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert!(run_isolation_command(&command).is_ok());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn isolation_link_claim_active_preserves_fields_and_duplicate_guards() {
        let (root, state, claim_id) = link_claim_fixture();
        let isolation_dir = state.join("contracts/isolations");
        let mut expected = load_isolations(&isolation_dir).0.remove(0);
        assert!(run_transition(&isolation_dir, &expected.id, IsolationStatus::Active, NOW).ok);
        expected.status = IsolationStatus::Active;
        assert!(run_isolation_command(&args(&[
            "isolation",
            "link-claim",
            "--root",
            root.to_str().unwrap(),
            "--id",
            &expected.id.0,
            "--claim",
            &claim_id,
            "--now-unix",
            &NOW.to_string()
        ]))
        .is_ok());
        expected.claim_id = Some(StableId(claim_id));
        assert_eq!(load_isolations(&isolation_dir).0, vec![expected]);
        assert!(
            !propose_ok(
                &isolation_dir,
                "alice",
                "alice/link",
                "../.forge-worktrees/alice/link"
            )
            .ok
        );
    }

    #[test]
    fn isolation_link_claim_rejections_preserve_exact_isolation_bytes() {
        for case in [
            "missing-isolation",
            "missing-claim",
            "expired-claim",
            "wrong-owner",
            "other-link",
            "merging",
            "merged",
            "abandoned",
            "duplicate-id",
            "noncanonical",
            "corrupt-document",
            "retry-expired",
            "retry-wrong-owner",
            "retry-terminal",
        ] {
            let (root, state, claim_id) = link_claim_fixture();
            let isolation_dir = state.join("contracts/isolations");
            let mut contract = load_isolations(&isolation_dir).0.remove(0);
            match case {
                "wrong-owner" | "retry-wrong-owner" => contract.agent_id = StableId("bob".into()),
                "other-link" => contract.claim_id = Some(StableId("another-claim".into())),
                "merging" => contract.status = IsolationStatus::Merging,
                "merged" | "retry-terminal" => contract.status = IsolationStatus::Merged,
                "abandoned" => contract.status = IsolationStatus::Abandoned,
                _ => {}
            }
            if case.starts_with("retry-") {
                contract.claim_id = Some(StableId(claim_id.clone()));
            }
            let path = save_isolation(&isolation_dir, &contract).unwrap();
            match case {
                "duplicate-id" => {
                    fs::copy(&path, isolation_dir.join("duplicate.yaml")).unwrap();
                }
                "noncanonical" => {
                    fs::rename(&path, isolation_dir.join("renamed.yaml")).unwrap();
                }
                "corrupt-document" => {
                    fs::write(isolation_dir.join("corrupt.yaml"), "[:invalid").unwrap();
                }
                _ => {}
            }
            let bytes: Vec<_> = fs::read_dir(&isolation_dir)
                .unwrap()
                .map(|entry| {
                    let path = entry.unwrap().path();
                    let raw = fs::read(&path).unwrap();
                    (path, raw)
                })
                .collect();
            let now = if matches!(case, "expired-claim" | "retry-expired") {
                NOW + 600
            } else {
                NOW
            };
            let id = if case == "missing-isolation" {
                "absent"
            } else {
                &contract.id.0
            };
            let claim = if case == "missing-claim" {
                "absent"
            } else {
                &claim_id
            };
            assert!(
                run_isolation_command(&args(&[
                    "isolation",
                    "link-claim",
                    "--root",
                    root.to_str().unwrap(),
                    "--id",
                    id,
                    "--claim",
                    claim,
                    "--now-unix",
                    &now.to_string()
                ]))
                .is_err(),
                "case {case}"
            );
            assert_eq!(
                fs::read_dir(&isolation_dir).unwrap().count(),
                bytes.len(),
                "case {case}"
            );
            for (path, before) in bytes {
                assert_eq!(fs::read(path).unwrap(), before, "case {case}");
            }
        }
    }

    #[test]
    fn isolation_link_claim_requires_explicit_selection_and_rejects_unknown_flags() {
        for flags in [
            vec![],
            vec!["--id", "isolation"],
            vec!["--claim", "claim"],
            vec!["--unknown"],
        ] {
            let mut command = args(&["isolation", "link-claim"]);
            command.extend(args(&flags));
            assert!(run_isolation_command(&command).is_err());
        }
        assert!(run_isolation_command(&args(&["isolation", "link-claim", "--help"])).is_ok());
    }

    // --- propose --------------------------------------------------------

    #[test]
    fn propose_valid_contract_succeeds_and_suggests_git() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        assert!(env.ok);
        assert_eq!(env.data.as_ref().unwrap().suggested_git_commands.len(), 2);
        assert!(env.data.as_ref().unwrap().suggested_git_commands[0].contains("worktree add"));
    }

    #[test]
    fn propose_illegal_branch_rejected() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice..s5", "../wt/a");
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
        assert!(env
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("illegal_branch_name"));
    }

    #[test]
    fn propose_duplicate_branch_rejected() {
        let d = dir();
        let _ = propose_ok(&d, "alice", "shared/x", "../wt/a");
        let env = propose_ok(&d, "bob", "shared/x", "../wt/b");
        assert!(!env.ok);
        assert_eq!(env.exit_code(), 2);
        assert!(env
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("duplicate_branch"));
    }

    #[test]
    fn propose_duplicate_worktree_path_rejected() {
        let d = dir();
        let _ = propose_ok(&d, "alice", "alice/s5", "../wt/shared");
        let env = propose_ok(&d, "bob", "bob/s6", "../wt/shared");
        assert!(!env.ok);
        assert!(env
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("duplicate_worktree_path"));
    }

    #[test]
    fn propose_disjoint_succeeds() {
        let d = dir();
        let _ = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let env = propose_ok(&d, "bob", "bob/s6", "../wt/b");
        assert!(env.ok);
    }

    // --- status ---------------------------------------------------------

    #[test]
    fn status_lists_active() {
        let d = dir();
        let _ = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let _ = propose_ok(&d, "bob", "bob/s6", "../wt/b");
        let env = run_status(&d, None);
        assert!(env.ok);
        assert_eq!(env.data.as_ref().unwrap().total, 2);
    }

    #[test]
    fn status_filters_by_agent() {
        let d = dir();
        let _ = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let _ = propose_ok(&d, "bob", "bob/s6", "../wt/b");
        let env = run_status(&d, Some(&StableId("alice".into())));
        assert_eq!(env.data.as_ref().unwrap().total, 1);
        assert_eq!(env.data.as_ref().unwrap().active[0].agent_id, "alice");
    }

    // --- merge-plan -----------------------------------------------------

    #[test]
    fn merge_plan_rebase_has_four_steps() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let id = StableId(env.data.unwrap().isolation.id.0);
        let plan = run_merge_plan(&d, &id, NOW);
        assert!(plan.ok);
        assert_eq!(plan.data.as_ref().unwrap().steps.len(), 4);
    }

    #[test]
    fn merge_plan_unknown_id_rejected() {
        let d = dir();
        let plan = run_merge_plan(&d, &StableId("nope".into()), NOW);
        assert!(!plan.ok);
        assert_eq!(plan.exit_code(), 3); // InvalidDecisionShape
    }

    // --- transition -----------------------------------------------------

    #[test]
    fn transition_proposed_to_active_succeeds() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let id = StableId(env.data.unwrap().isolation.id.0);
        let t = run_transition(&d, &id, IsolationStatus::Active, NOW);
        assert!(t.ok);
        assert_eq!(
            t.data.as_ref().unwrap().isolation.status,
            IsolationStatus::Active
        );
    }

    #[test]
    fn transition_illegal_rejected() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let id = StableId(env.data.unwrap().isolation.id.0);
        // Proposed -> Merging is illegal (must go through Active first)
        let t = run_transition(&d, &id, IsolationStatus::Merging, NOW);
        assert!(!t.ok);
        assert_eq!(t.exit_code(), 2);
        assert!(t
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("illegal_transition"));
    }

    #[test]
    fn transition_to_merging_suggests_merge_commands() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let id = StableId(env.data.unwrap().isolation.id.0);
        let _ = run_transition(&d, &id, IsolationStatus::Active, NOW);
        let t = run_transition(&d, &id, IsolationStatus::Merging, NOW);
        assert!(t.ok);
        assert!(!t.data.as_ref().unwrap().suggested_git_commands.is_empty());
        assert!(t
            .data
            .as_ref()
            .unwrap()
            .suggested_git_commands
            .iter()
            .any(|c| c.contains("rebase")));
    }

    #[test]
    fn merged_contract_releases_branch_for_reuse() {
        let d = dir();
        let env = propose_ok(&d, "alice", "alice/s5", "../wt/a");
        let id = StableId(env.data.unwrap().isolation.id.0);
        let _ = run_transition(&d, &id, IsolationStatus::Active, NOW);
        let _ = run_transition(&d, &id, IsolationStatus::Merging, NOW);
        let _ = run_transition(&d, &id, IsolationStatus::Merged, NOW);
        // Now bob can claim the same branch (alice's is terminal)
        let env2 = propose_ok(&d, "bob", "alice/s5", "../wt/a2");
        assert!(env2.ok, "merged isolation releases its branch + path");
    }

    // --- parsers --------------------------------------------------------

    #[test]
    fn parse_merge_policy_round_trips() {
        assert_eq!(parse_merge_policy("Rebase").unwrap(), MergePolicy::Rebase);
        assert_eq!(parse_merge_policy("SQUASH").unwrap(), MergePolicy::Squash);
        assert!(parse_merge_policy("bogus").is_err());
    }

    #[test]
    fn parse_status_round_trips() {
        assert_eq!(parse_status("active").unwrap(), IsolationStatus::Active);
        assert!(parse_status("bogus").is_err());
    }

    // --- IO robustness --------------------------------------------------

    #[test]
    fn load_missing_dir_is_empty_not_error() {
        let d = std::env::temp_dir().join("iso-nonexistent-xyz");
        let (contracts, errs) = load_isolations(&d);
        assert!(contracts.is_empty());
        assert!(errs.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let d = dir();
        let c = IsolationContract {
            id: StableId("iso-x".into()),
            agent_id: StableId("alice".into()),
            branch_name: "alice/s5".into(),
            worktree_path: RepoPath("../wt/a".into()),
            base_ref: "main".into(),
            created_at: "2027-01-01T00:00:00Z".into(),
            status: IsolationStatus::Active,
            merge_policy: MergePolicy::Merge,
            claim_id: None,
        };
        save_isolation(&d, &c).unwrap();
        let (loaded, errs) = load_isolations(&d);
        assert!(errs.is_empty());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0], c);
    }
    #[test]
    fn retained_isolation_authority_projects_exact_sorted_bytes_and_blocks_producer() {
        let d = dir();
        let make_contract = |id: &str, agent: &str, branch: &str, path: &str| IsolationContract {
            id: StableId(id.to_owned()),
            agent_id: StableId(agent.to_owned()),
            branch_name: branch.to_owned(),
            worktree_path: RepoPath(path.to_owned()),
            base_ref: "main".to_owned(),
            created_at: "2027-01-01T00:00:00Z".to_owned(),
            status: IsolationStatus::Active,
            merge_policy: MergePolicy::Merge,
            claim_id: None,
        };
        save_isolation(&d, &make_contract("iso-z", "zed", "zed/s5", "../wt/z")).unwrap();
        save_isolation(&d, &make_contract("iso-a", "alice", "alice/s5", "../wt/a")).unwrap();

        let locked = acquire_isolation_contracts_authority(&d).expect("retained authority");
        let snapshot = snapshot_isolation_contracts_under_authority(&locked).expect("projection");
        assert_eq!(snapshot.entries().len(), 2);
        assert_eq!(snapshot.entries()[0].contract().id.0, "iso-a");
        for entry in snapshot.entries() {
            assert_eq!(
                entry.raw(),
                std::fs::read(d.join(entry.relative_path())).unwrap()
            );
            assert_eq!(
                entry.raw_sha256(),
                format!("sha256:{:x}", Sha256::digest(entry.raw()))
            );
        }
        let Err(error) = acquire_isolation_contracts_authority(&d) else {
            panic!("producer lock must contend");
        };
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }

    // --- review S4.6 C1 / M2 regression ----------------------------------

    #[test]
    fn shell_quote_escapes_metacharacters() {
        // The exact strings that would inject a command MUST come out fully
        // single-quoted so copy-paste cannot execute them.
        assert_eq!(shell_quote("main"), "'main'");
        assert_eq!(shell_quote("main;touch pwned"), "'main;touch pwned'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn suggested_commands_shell_quote_every_field() {
        // Even though the engine now rejects metachars (defense in depth),
        // a value reaching this fn via a hand-crafted contract must STILL be
        // quoted — never emit a raw interpolation.
        let c = IsolationContract {
            id: StableId("i".into()),
            agent_id: StableId("alice".into()),
            branch_name: "x;y".into(),
            worktree_path: RepoPath("p;q".into()),
            base_ref: "m;n".into(),
            created_at: "2027-01-01T00:00:00Z".into(),
            status: IsolationStatus::Active,
            merge_policy: MergePolicy::Rebase,
            claim_id: None,
        };
        let cmds = suggested_worktree_commands(&c);
        // The injected `;` must be INSIDE single quotes, never a shell break.
        assert!(cmds[0].contains("'x;y'") || cmds[0].contains("'x;y'"));
        assert!(cmds[0].contains("'p;q'"));
        assert!(cmds[0].contains("'m;n'"));
        // Every semicolon in the output sits inside a single-quoted token:
        // 3 values × 1 ';' each = 3 total, all quoted. (Counts match ⇒ none
        // are shell-breaking.)
        let total_semicolons = cmds[0].matches(';').count();
        let quoted_values = cmds[0].matches("'x;y'").count()
            + cmds[0].matches("'p;q'").count()
            + cmds[0].matches("'m;n'").count();
        assert_eq!(
            total_semicolons, quoted_values,
            "every semicolon must be inside a quoted value"
        );
    }

    #[test]
    fn isolation_status_serializes_snake_case() {
        // M2: on-disk YAML must use lowercase to match status_str/parse_status.
        let yaml = yaml_serde::to_string(&IsolationStatus::Active).unwrap();
        assert!(yaml.contains("active"), "got: {yaml}");
        assert!(!yaml.contains("Active"));
        let parsed: IsolationStatus = yaml_serde::from_str("active").unwrap();
        assert_eq!(parsed, IsolationStatus::Active);
    }

    #[test]
    fn merge_policy_serializes_snake_case() {
        let yaml = yaml_serde::to_string(&MergePolicy::Squash).unwrap();
        assert!(yaml.contains("squash"));
        let parsed: MergePolicy = yaml_serde::from_str("squash").unwrap();
        assert_eq!(parsed, MergePolicy::Squash);
    }

    #[test]
    fn isolation_usage_projects_command_surface_lines() {
        let mut usage = String::from("forge-core isolation <subcommand> [options]");
        for line in COMMAND_ISOLATION.local_usage_lines() {
            usage.push('\n');
            usage.push_str("  ");
            usage.push_str(line);
        }

        assert!(
            usage.starts_with("forge-core isolation <subcommand> [options]"),
            "isolation usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_ISOLATION.usage_lines {
            let subcommand_usage = COMMAND_ISOLATION.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "isolation usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(
            isolation_subcommand_hint(),
            "propose | link-claim | status | merge-plan | transition"
        );
    }

    #[test]
    fn isolation_subcommand_help_lookup_projects_full_command_surface_lines() {
        for subcommand in [
            "propose",
            "link-claim",
            "status",
            "merge-plan",
            "transition",
        ] {
            let usage = isolation_command_surface_usage_line_for(subcommand);
            assert_eq!(
                Some(usage),
                COMMAND_ISOLATION.usage_line_for_subcommand(subcommand),
                "isolation {subcommand} help should come from the Command Surface"
            );
        }
    }

    #[test]
    fn isolation_status_accepts_explicit_json_mode() {
        let d = dir();
        let d_str = d.to_str().expect("isolation dir path utf-8");
        let result = run_isolation_status(&args(&["--json", "--isolation-dir", d_str]));

        assert!(result.is_ok(), "explicit --json should parse");
    }

    #[test]
    fn isolation_status_rejects_unknown_argument() {
        let result = run_isolation_status(&args(&["--definitely-unknown"]));

        assert!(
            result.is_err(),
            "unknown isolation status arguments should fail closed"
        );
    }
}
