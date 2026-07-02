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
use crate::io_util::{atomic_write, DirLock};
use forge_core_contracts::common::StableId;
use forge_core_contracts::isolation::{
    IsolationContract, IsolationContractDocument, IsolationError, IsolationStatus, MergePlan,
    MergePolicy,
};
use forge_core_contracts::{CliEnvelope, ExitReason, RepoPath, ENVELOPE_SCHEMA_VERSION};
use forge_core_decisions::isolation::{
    detect_isolation_conflict, propose_merge, transition_status, validate_isolation_contract,
};
use std::path::{Path, PathBuf};

const LOCKFILE: &str = ".forge-isolation.lock";

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
    // 2) collision against existing live contracts (under lock)
    let lock = match DirLock::acquire(isolation_dir, LOCKFILE) {
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
    // 3) persist
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
    let lock = match DirLock::acquire(isolation_dir, LOCKFILE) {
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
        "status" => run_isolation_status(&args[2..]),
        "merge-plan" => run_isolation_merge_plan(&args[2..]),
        "transition" => run_isolation_transition(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("forge-core isolation <subcommand> [options]");
            println!("  propose [--root <path>] [--allow-bootstrap-core] --agent <id> --branch <name> --worktree-path <path> --base-ref <ref> [--id <isolation-id>] [--merge-policy rebase|merge|squash] [--claim <claim-id>] [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  status [--root <path>] [--allow-bootstrap-core] [--agent <id>] [--isolation-dir <path>] [--no-json]");
            println!("  merge-plan [--root <path>] [--allow-bootstrap-core] --id <isolation-id> [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  transition [--root <path>] [--allow-bootstrap-core] --id <isolation-id> --to proposed|active|merging|merged|abandoned [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
            println!("  Defaults: without --isolation-dir, resolves --root as a Forge project and uses <state_root>/contracts/isolations; --isolation-dir is an explicit override.");
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core isolation: unknown subcommand '{other}'. Try: propose | status | merge-plan | transition"
        ))),
    }
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
    allow_bootstrap_core: bool,
    want_json: bool,
) -> Result<PathBuf, ExitError> {
    if let Some(isolation_dir) = isolation_dir {
        return Ok(isolation_dir);
    }

    match crate::project_cmd::resolve_project(root, allow_bootstrap_core) {
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
    let mut allow_bootstrap_core = false;
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
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
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
            "--help" | "-h" => {
                println!("forge-core isolation propose [--root <path>] [--allow-bootstrap-core] --agent <id> --branch <name> --worktree-path <path> --base-ref <ref> [--id <id>] [--merge-policy rebase|merge|squash] [--claim <claim-id>] [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
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
    let isolation_dir = resolve_isolation_dir_or_err(
        "isolation.propose",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = run_propose(
        &isolation_dir,
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
    let mut allow_bootstrap_core = false;
    let mut want_json = true;
    let mut agent: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = PathBuf::from(require_value_or_err(args, idx, "root")?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
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
            "--help" | "-h" => {
                println!("forge-core isolation status [--root <path>] [--allow-bootstrap-core] [--agent <id>] [--isolation-dir <path>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    let isolation_dir = resolve_isolation_dir_or_err(
        "isolation.status",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
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
    let mut allow_bootstrap_core = false;
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
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
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
            "--help" | "-h" => {
                println!("forge-core isolation merge-plan [--root <path>] [--allow-bootstrap-core] --id <isolation-id> [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
        }
        idx += 1;
    }
    if id.is_empty() {
        eprintln!("isolation merge-plan: --id <isolation-id> is required");
        return Err(ExitError::invalid_value(
            "isolation merge-plan: --id <isolation-id> is required",
        ));
    }
    let isolation_dir = resolve_isolation_dir_or_err(
        "isolation.merge-plan",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
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
    let mut allow_bootstrap_core = false;
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
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
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
            "--help" | "-h" => {
                println!("forge-core isolation transition [--root <path>] [--allow-bootstrap-core] --id <isolation-id> --to proposed|active|merging|merged|abandoned [--isolation-dir <path>] [--now-unix <epoch>] [--no-json]");
                println!("  Without --isolation-dir, resolves --root and uses <state_root>/contracts/isolations; --isolation-dir preserves the explicit override.");
                return Ok(());
            }
            _ => {}
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
    let isolation_dir = resolve_isolation_dir_or_err(
        "isolation.transition",
        isolation_dir,
        &root,
        allow_bootstrap_core,
        want_json,
    )?;
    let env = run_transition(
        &isolation_dir,
        &StableId(id),
        to,
        resolve_now_unix(now_unix),
    );
    emit_envelope_or_err("isolation", env, want_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
}
