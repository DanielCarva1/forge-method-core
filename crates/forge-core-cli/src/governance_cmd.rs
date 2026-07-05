//! `forge-core governance` — the CLI surface for the F07 arbitration ledger
//! (F07.6).
//!
//! Four subcommands wrap the `forge-core-governance` PEP (ADR-0007):
//! - `record`    — persist a freshly-detected `ConflictContract` (from a YAML
//!   `--conflict-file`) to the append-only ledger. Calls
//!   `forge_core_governance::record`. Idempotent on `conflict_id`.
//! - `conflicts` — list conflicts in the ledger, optionally filtered by
//!   `--status`. Calls `list`.
//! - `arbitrate` — move a `Pending` conflict to `Resolved`, gated by the
//!   governance policy (`can_arbitrate`). Calls `arbitrate`.
//! - `escalate`  — move a `Pending` conflict to `Escalated`, gated by the
//!   policy. Calls `escalate`.
//!
//! State writes go to `<state_root>/governance/` (resolved via `resolve_project`,
//! same pattern as `memory`). No claim-governance is required — state-dir
//! writes are `Ungoverned` by the classifier and gated by the governance file
//! lock instead.
//!
//! Output: standard [`CliEnvelope`] dual-output (JSON for agents, text for
//! humans), mirroring `memory_cmd.rs`.

use std::path::{Path, PathBuf};

use forge_core_command_surface::COMMAND_GOVERNANCE;
use forge_core_contracts::{
    CliEnvelope, ConflictContract, ConflictResolutionState, ExitReason, GovernancePolicy,
    PrincipalId, ResolutionDecision, StableId,
};
use forge_core_governance::{
    arbitrate, escalate, list, record, ArbitrateStatus, EscalateStatus, RecordStatus,
};

use crate::cli_error::ExitError;

const GOVERNANCE_COMMAND: &str = "governance";
const RECORD_COMMAND: &str = "governance record";
const CONFLICTS_COMMAND: &str = "governance conflicts";
const ARBITRATE_COMMAND: &str = "governance arbitrate";
const ESCALATE_COMMAND: &str = "governance escalate";

/// Parse and run `forge-core governance <subcommand>`.
///
/// # Errors
///
/// Returns `ExitError::usage` (via envelope emission) when the subcommand is
/// unknown or argument parsing fails.
pub fn run_governance_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "record" => run_record(&args[2..]),
        "conflicts" => run_conflicts(&args[2..]),
        "arbitrate" => run_arbitrate(&args[2..]),
        "escalate" => run_escalate(&args[2..]),
        "--help" | "-h" | "help" => {
            print_governance_usage();
            Ok(())
        }
        other => {
            let want_json = json_output_unless_text_selected(&args[2..]);
            emit_err(
                GOVERNANCE_COMMAND,
                &format!(
                    "unknown subcommand '{other}'. Try: {hint}",
                    hint = governance_subcommand_hint()
                ),
                want_json,
            )
        }
    }
}

fn print_governance_usage() {
    println!("forge-core governance <subcommand> [options]");
    for line in COMMAND_GOVERNANCE.local_usage_lines() {
        println!("  {line}");
    }
    println!();
    println!("  State writes land under <state_root>/governance/ (resolved from --root). --governance-dir overrides the directory.");
}

fn governance_subcommand_hint() -> String {
    COMMAND_GOVERNANCE.concrete_subcommand_hint()
}

fn governance_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_GOVERNANCE
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core governance <subcommand> [options]")
}

// --- shared resolution -------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct GovernanceResolveError {
    message: String,
}

impl GovernanceResolveError {
    #[must_use]
    fn message(&self) -> String {
        self.message.clone()
    }
}

/// Resolve the governance state directory. The PEP writes the log under this
/// root as `<dir>/governance/conflicts.ndjson`, so we resolve up to `<state_root>`
/// (the PEP appends the `governance/` segment itself). `--governance-dir`
/// overrides to act as the state root directly.
fn resolve_governance_dir(
    root: Option<&str>,
    allow_bootstrap_core: bool,
    governance_dir: Option<&str>,
) -> Result<PathBuf, GovernanceResolveError> {
    if let Some(dir) = governance_dir {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path).map_err(|source| GovernanceResolveError {
            message: format!(
                "cannot create --governance-dir '{}': {source}",
                path.display()
            ),
        })?;
        return Ok(path);
    }
    let root_str = root.unwrap_or(".");
    let root_path = PathBuf::from(root_str);
    let project = crate::project_cmd::resolve_project(&root_path, allow_bootstrap_core).map_err(
        |source| GovernanceResolveError {
            message: format!("cannot resolve Forge project from --root '{root_str}': {source}"),
        },
    )?;
    let state_root = PathBuf::from(&project.state_root);
    if !state_root.is_dir() {
        return Err(GovernanceResolveError {
            message: format!(
                "resolved Forge state_root is not a directory: {}; create the sidecar .forge-method directory or pass --governance-dir",
                state_root.display()
            ),
        });
    }
    Ok(state_root)
}

// --- common option fields ----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CommonOptions {
    root: Option<String>,
    allow_bootstrap_core: bool,
    governance_dir: Option<String>,
    want_json: bool,
}

enum CommonFlag {
    Consumed,
    Unknown(String),
}

fn parse_common_flag(
    args: &[String],
    idx: &mut usize,
    common: &mut CommonOptions,
) -> Result<CommonFlag, GovernanceParseError> {
    let want_json = common.want_json;
    match args[*idx].as_str() {
        "--root" => {
            *idx += 1;
            let value = require_value(args, *idx, "root", want_json)?;
            common.root = Some(value);
            Ok(CommonFlag::Consumed)
        }
        "--allow-bootstrap-core" => {
            common.allow_bootstrap_core = true;
            Ok(CommonFlag::Consumed)
        }
        "--governance-dir" => {
            *idx += 1;
            let value = require_value(args, *idx, "governance-dir", want_json)?;
            common.governance_dir = Some(value);
            Ok(CommonFlag::Consumed)
        }
        "--no-json" | "--text" => {
            common.want_json = false;
            Ok(CommonFlag::Consumed)
        }
        "--json" => Ok(CommonFlag::Consumed),
        other => Ok(CommonFlag::Unknown(other.to_string())),
    }
}

// --- record ------------------------------------------------------------------

fn run_record(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_record_args(args) {
        Ok(GovernanceParseOutcome::Help) => {
            println!("{}", governance_command_surface_usage_line_for("record"));
            return Ok(());
        }
        Ok(GovernanceParseOutcome::Run(opts)) => opts,
        Err(error) => return emit_err(RECORD_COMMAND, &error.message(), error.want_json()),
    };
    let conflict = match load_conflict_file(&outcome.conflict_file) {
        Ok(conflict) => conflict,
        Err(error) => return emit_err(RECORD_COMMAND, &error.message(), outcome.common.want_json),
    };
    let governance_dir = match resolve_governance_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.governance_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return emit_err(RECORD_COMMAND, &error.message(), outcome.common.want_json),
    };

    let result = record(&governance_dir, conflict);
    let env: CliEnvelope<serde_json::Value> = match result.status {
        RecordStatus::Recorded { sequence } => CliEnvelope::ok(
            RECORD_COMMAND,
            serde_json::to_value(RecordOkData {
                conflict_id: result.conflict_id.0,
                sequence,
            })
            .expect("serialize record ok"),
        ),
        RecordStatus::AlreadyRecorded => CliEnvelope::ok(
            RECORD_COMMAND,
            serde_json::to_value(RecordOkData {
                conflict_id: result.conflict_id.0,
                sequence: 0,
            })
            .expect("serialize record already"),
        ),
        RecordStatus::StoreError(error) => CliEnvelope::err(
            RECORD_COMMAND,
            ExitReason::Conflict,
            format!("governance store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct RecordOkData {
    conflict_id: String,
    sequence: u64,
}

// --- conflicts (list) --------------------------------------------------------

fn run_conflicts(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_conflicts_args(args) {
        Ok(GovernanceParseOutcome::Help) => {
            println!("{}", governance_command_surface_usage_line_for("conflicts"));
            return Ok(());
        }
        Ok(GovernanceParseOutcome::Run(opts)) => opts,
        Err(error) => return emit_err(CONFLICTS_COMMAND, &error.message(), error.want_json()),
    };
    let governance_dir = match resolve_governance_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.governance_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => {
            return emit_err(
                CONFLICTS_COMMAND,
                &error.message(),
                outcome.common.want_json,
            )
        }
    };

    // Read ALL conflicts (no value-exact filter — the filter is a category tag),
    // then filter by the requested variant tag in-process. A `--status resolved`
    // user wants every resolved conflict regardless of which arbiter/decision.
    let result = match list(&governance_dir, None) {
        Ok(projection) => projection,
        Err(error) => {
            return emit_err(
                CONFLICTS_COMMAND,
                &format!("governance store error: {error}"),
                outcome.common.want_json,
            );
        }
    };
    let conflicts: Vec<ConflictView> = result
        .conflicts
        .into_values()
        .filter(|c| match &outcome.status {
            None => true,
            Some(tag) => resolution_tag(&c.resolution) == tag,
        })
        .map(conflict_view)
        .collect();
    let env = CliEnvelope::ok(
        CONFLICTS_COMMAND,
        ConflictsOkData {
            count: conflicts.len(),
            conflicts,
        },
    );
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

/// The variant tag of a `ConflictResolutionState` ("pending" / "resolved" /
/// "escalated"). Used for `--status` category filtering: a `--status resolved`
/// request must match ALL resolved conflicts regardless of which arbiter or
/// decision, so a value-exact comparison would be wrong.
#[must_use]
fn resolution_tag(state: &ConflictResolutionState) -> &str {
    match state {
        ConflictResolutionState::Pending => "pending",
        ConflictResolutionState::Resolved { .. } => "resolved",
        ConflictResolutionState::Escalated => "escalated",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ConflictsOkData {
    count: usize,
    conflicts: Vec<ConflictView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ConflictView {
    conflict_id: String,
    principal_a: String,
    principal_b: String,
    contested_scope: String,
    detection_reason: String,
    resolution: String,
}

fn conflict_view(c: ConflictContract) -> ConflictView {
    let resolution = match c.resolution {
        ConflictResolutionState::Pending => "pending".into(),
        ConflictResolutionState::Resolved { .. } => "resolved".into(),
        ConflictResolutionState::Escalated => "escalated".into(),
    };
    ConflictView {
        conflict_id: c.conflict_id.0,
        principal_a: c.principal_a.0,
        principal_b: c.principal_b.0,
        contested_scope: c.contested_scope.target.0,
        detection_reason: serde_json::to_value(c.detection_reason)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        resolution,
    }
}

// --- arbitrate ---------------------------------------------------------------

fn run_arbitrate(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_arbitrate_args(args) {
        Ok(GovernanceParseOutcome::Help) => {
            println!("{}", governance_command_surface_usage_line_for("arbitrate"));
            return Ok(());
        }
        Ok(GovernanceParseOutcome::Run(opts)) => opts,
        Err(error) => return emit_err(ARBITRATE_COMMAND, &error.message(), error.want_json()),
    };
    let policy = match load_policy_file(&outcome.policy_file) {
        Ok(policy) => policy,
        Err(error) => {
            return emit_err(
                ARBITRATE_COMMAND,
                &error.message(),
                outcome.common.want_json,
            )
        }
    };
    let governance_dir = match resolve_governance_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.governance_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => {
            return emit_err(
                ARBITRATE_COMMAND,
                &error.message(),
                outcome.common.want_json,
            )
        }
    };

    // Exactly one decision flag must be set.
    let decision = match (
        outcome.awarded_to,
        outcome.both_released,
        outcome.split_scope,
    ) {
        (Some(principal), false, false) => ResolutionDecision::AwardedTo(PrincipalId(principal)),
        (None, true, false) => ResolutionDecision::BothReleased,
        (None, false, true) => ResolutionDecision::SplitScope,
        (None, false, false) => {
            return emit_err(
                ARBITRATE_COMMAND,
                "a decision is required: pass exactly one of --awarded-to, --both-released, --split-scope",
                outcome.common.want_json,
            );
        }
        _ => {
            return emit_err(
                ARBITRATE_COMMAND,
                "pass exactly ONE decision flag (--awarded-to, --both-released, --split-scope)",
                outcome.common.want_json,
            );
        }
    };

    let result = arbitrate(
        &governance_dir,
        StableId(outcome.conflict_id.clone()),
        &PrincipalId(outcome.arbiter.clone()),
        decision,
        &policy,
    );
    let env: CliEnvelope<serde_json::Value> = match result.status {
        ArbitrateStatus::Resolved { sequence } => CliEnvelope::ok(
            ARBITRATE_COMMAND,
            serde_json::to_value(ArbitrateOkData {
                conflict_id: result.conflict_id.0,
                sequence,
            })
            .expect("serialize arbitrate ok"),
        ),
        ArbitrateStatus::DeniedByGate => CliEnvelope::reject(
            ARBITRATE_COMMAND,
            ExitReason::RejectedByGate,
            "arbitration gate denied: arbiter is not in authorized_reviewers",
            serde_json::to_value(ArbitrateDenialData {
                arbiter: outcome.arbiter,
            })
            .expect("serialize denial"),
        ),
        ArbitrateStatus::ConflictNotFound => CliEnvelope::err(
            ARBITRATE_COMMAND,
            ExitReason::RejectedByGate,
            format!("conflict '{}' not found in the ledger", outcome.conflict_id),
        ),
        ArbitrateStatus::NotPending => CliEnvelope::err(
            ARBITRATE_COMMAND,
            ExitReason::RejectedByGate,
            format!(
                "conflict '{}' is not pending (already resolved or escalated)",
                outcome.conflict_id
            ),
        ),
        ArbitrateStatus::StoreError(error) => CliEnvelope::err(
            ARBITRATE_COMMAND,
            ExitReason::Conflict,
            format!("governance store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ArbitrateOkData {
    conflict_id: String,
    sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ArbitrateDenialData {
    arbiter: String,
}

// --- escalate ----------------------------------------------------------------

fn run_escalate(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_escalate_args(args) {
        Ok(GovernanceParseOutcome::Help) => {
            println!("{}", governance_command_surface_usage_line_for("escalate"));
            return Ok(());
        }
        Ok(GovernanceParseOutcome::Run(opts)) => opts,
        Err(error) => return emit_err(ESCALATE_COMMAND, &error.message(), error.want_json()),
    };
    let policy = match load_policy_file(&outcome.policy_file) {
        Ok(policy) => policy,
        Err(error) => {
            return emit_err(ESCALATE_COMMAND, &error.message(), outcome.common.want_json)
        }
    };
    let governance_dir = match resolve_governance_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.governance_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => {
            return emit_err(ESCALATE_COMMAND, &error.message(), outcome.common.want_json)
        }
    };

    let result = escalate(
        &governance_dir,
        StableId(outcome.conflict_id.clone()),
        &PrincipalId(outcome.principal.clone()),
        &policy,
    );
    let env: CliEnvelope<serde_json::Value> = match result.status {
        EscalateStatus::Escalated { sequence } => CliEnvelope::ok(
            ESCALATE_COMMAND,
            serde_json::to_value(EscalateOkData {
                conflict_id: result.conflict_id.0,
                sequence,
            })
            .expect("serialize escalate ok"),
        ),
        EscalateStatus::DeniedByGate => CliEnvelope::reject(
            ESCALATE_COMMAND,
            ExitReason::RejectedByGate,
            "arbitration gate denied: principal is not in authorized_reviewers",
            serde_json::to_value(ArbitrateDenialData {
                arbiter: outcome.principal,
            })
            .expect("serialize denial"),
        ),
        EscalateStatus::ConflictNotFound => CliEnvelope::err(
            ESCALATE_COMMAND,
            ExitReason::RejectedByGate,
            format!("conflict '{}' not found in the ledger", outcome.conflict_id),
        ),
        EscalateStatus::NotPending => CliEnvelope::err(
            ESCALATE_COMMAND,
            ExitReason::RejectedByGate,
            format!(
                "conflict '{}' is not pending (already resolved or escalated)",
                outcome.conflict_id
            ),
        ),
        EscalateStatus::StoreError(error) => CliEnvelope::err(
            ESCALATE_COMMAND,
            ExitReason::Conflict,
            format!("governance store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct EscalateOkData {
    conflict_id: String,
    sequence: u64,
}

// --- arg parsing (per subcommand) --------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum GovernanceParseOutcome<T> {
    Help,
    Run(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GovernanceParseError {
    MissingValue {
        flag: &'static str,
        want_json: bool,
    },
    FlagAsValue {
        flag: &'static str,
        value: String,
        want_json: bool,
    },
    MissingRequired {
        flag: &'static str,
        want_json: bool,
    },
    InvalidStatus {
        value: String,
        want_json: bool,
    },
    UnknownArgument {
        argument: String,
        want_json: bool,
    },
}

impl GovernanceParseError {
    #[must_use]
    fn want_json(&self) -> bool {
        match self {
            Self::MissingValue { want_json, .. }
            | Self::FlagAsValue { want_json, .. }
            | Self::MissingRequired { want_json, .. }
            | Self::InvalidStatus { want_json, .. }
            | Self::UnknownArgument { want_json, .. } => *want_json,
        }
    }

    #[must_use]
    fn message(&self) -> String {
        match self {
            Self::MissingValue { flag, .. } => format!("--{flag} requires a value"),
            Self::FlagAsValue { flag, value, .. } => {
                format!("--{flag} requires a value, got another flag '{value}'")
            }
            Self::MissingRequired { flag, .. } => format!("--{flag} is required"),
            Self::InvalidStatus { value, .. } => {
                format!("--status must be one of pending|resolved|escalated, got '{value}'")
            }
            Self::UnknownArgument { argument, .. } => format!("unknown argument '{argument}'"),
        }
    }
}

fn require_value(
    args: &[String],
    idx: usize,
    flag: &'static str,
    want_json: bool,
) -> Result<String, GovernanceParseError> {
    match args.get(idx) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(GovernanceParseError::FlagAsValue {
                flag,
                value: value.clone(),
                want_json,
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(GovernanceParseError::MissingValue { flag, want_json }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordOptions {
    common: CommonOptions,
    conflict_file: PathBuf,
}

fn parse_record_args(
    args: &[String],
) -> Result<GovernanceParseOutcome<RecordOptions>, GovernanceParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut conflict_file: Option<PathBuf> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--conflict-file" => {
                    idx += 1;
                    conflict_file = Some(PathBuf::from(require_value(
                        args,
                        idx,
                        "conflict-file",
                        common.want_json,
                    )?));
                }
                "--help" | "-h" => return Ok(GovernanceParseOutcome::Help),
                other => {
                    return Err(GovernanceParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let conflict_file = conflict_file.ok_or(GovernanceParseError::MissingRequired {
        flag: "conflict-file",
        want_json: common.want_json,
    })?;
    Ok(GovernanceParseOutcome::Run(RecordOptions {
        common,
        conflict_file,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConflictsOptions {
    common: CommonOptions,
    /// The variant tag to filter by ("pending" / "resolved" / "escalated"), or
    /// None for all. Stored as a String (not the enum) because the filter is a
    /// category, not a value-exact match.
    status: Option<String>,
}

fn parse_conflicts_args(
    args: &[String],
) -> Result<GovernanceParseOutcome<ConflictsOptions>, GovernanceParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut status: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--status" => {
                    idx += 1;
                    let raw = require_value(args, idx, "status", common.want_json)?;
                    if !matches!(raw.as_str(), "pending" | "resolved" | "escalated") {
                        return Err(GovernanceParseError::InvalidStatus {
                            value: raw,
                            want_json: common.want_json,
                        });
                    }
                    status = Some(raw);
                }
                "--help" | "-h" => return Ok(GovernanceParseOutcome::Help),
                other => {
                    return Err(GovernanceParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    Ok(GovernanceParseOutcome::Run(ConflictsOptions {
        common,
        status,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArbitrateOptions {
    common: CommonOptions,
    conflict_id: String,
    policy_file: PathBuf,
    arbiter: String,
    awarded_to: Option<String>,
    both_released: bool,
    split_scope: bool,
}

fn parse_arbitrate_args(
    args: &[String],
) -> Result<GovernanceParseOutcome<ArbitrateOptions>, GovernanceParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut conflict_id: Option<String> = None;
    let mut policy_file: Option<PathBuf> = None;
    let mut arbiter: Option<String> = None;
    let mut awarded_to: Option<String> = None;
    let mut both_released = false;
    let mut split_scope = false;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--conflict-id" => {
                    idx += 1;
                    conflict_id = Some(require_value(args, idx, "conflict-id", common.want_json)?);
                }
                "--policy-file" => {
                    idx += 1;
                    policy_file = Some(PathBuf::from(require_value(
                        args,
                        idx,
                        "policy-file",
                        common.want_json,
                    )?));
                }
                "--arbiter" => {
                    idx += 1;
                    arbiter = Some(require_value(args, idx, "arbiter", common.want_json)?);
                }
                "--awarded-to" => {
                    idx += 1;
                    awarded_to = Some(require_value(args, idx, "awarded-to", common.want_json)?);
                }
                "--both-released" => {
                    both_released = true;
                }
                "--split-scope" => {
                    split_scope = true;
                }
                "--help" | "-h" => return Ok(GovernanceParseOutcome::Help),
                other => {
                    return Err(GovernanceParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let conflict_id = conflict_id.ok_or(GovernanceParseError::MissingRequired {
        flag: "conflict-id",
        want_json: common.want_json,
    })?;
    let policy_file = policy_file.ok_or(GovernanceParseError::MissingRequired {
        flag: "policy-file",
        want_json: common.want_json,
    })?;
    let arbiter = arbiter.ok_or(GovernanceParseError::MissingRequired {
        flag: "arbiter",
        want_json: common.want_json,
    })?;
    Ok(GovernanceParseOutcome::Run(ArbitrateOptions {
        common,
        conflict_id,
        policy_file,
        arbiter,
        awarded_to,
        both_released,
        split_scope,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EscalateOptions {
    common: CommonOptions,
    conflict_id: String,
    policy_file: PathBuf,
    principal: String,
}

fn parse_escalate_args(
    args: &[String],
) -> Result<GovernanceParseOutcome<EscalateOptions>, GovernanceParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut conflict_id: Option<String> = None;
    let mut policy_file: Option<PathBuf> = None;
    let mut principal: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--conflict-id" => {
                    idx += 1;
                    conflict_id = Some(require_value(args, idx, "conflict-id", common.want_json)?);
                }
                "--policy-file" => {
                    idx += 1;
                    policy_file = Some(PathBuf::from(require_value(
                        args,
                        idx,
                        "policy-file",
                        common.want_json,
                    )?));
                }
                "--principal" => {
                    idx += 1;
                    principal = Some(require_value(args, idx, "principal", common.want_json)?);
                }
                "--help" | "-h" => return Ok(GovernanceParseOutcome::Help),
                other => {
                    return Err(GovernanceParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let conflict_id = conflict_id.ok_or(GovernanceParseError::MissingRequired {
        flag: "conflict-id",
        want_json: common.want_json,
    })?;
    let policy_file = policy_file.ok_or(GovernanceParseError::MissingRequired {
        flag: "policy-file",
        want_json: common.want_json,
    })?;
    let principal = principal.ok_or(GovernanceParseError::MissingRequired {
        flag: "principal",
        want_json: common.want_json,
    })?;
    Ok(GovernanceParseOutcome::Run(EscalateOptions {
        common,
        conflict_id,
        policy_file,
        principal,
    }))
}

// --- file loaders ------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadError {
    message: String,
}

impl LoadError {
    #[must_use]
    fn message(&self) -> String {
        self.message.clone()
    }
}

fn load_conflict_file(path: &Path) -> Result<ConflictContract, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError {
        message: format!("cannot read conflict file '{}': {source}", path.display()),
    })?;
    yaml_serde::from_str::<ConflictContract>(&text).map_err(|source| LoadError {
        message: format!(
            "conflict file '{}' is not a valid ConflictContract YAML: {source}",
            path.display()
        ),
    })
}

fn load_policy_file(path: &Path) -> Result<GovernancePolicy, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError {
        message: format!("cannot read policy file '{}': {source}", path.display()),
    })?;
    yaml_serde::from_str::<GovernancePolicy>(&text).map_err(|source| LoadError {
        message: format!(
            "policy file '{}' is not a valid GovernancePolicy YAML: {source}",
            path.display()
        ),
    })
}

// --- emit helpers (mirror memory_cmd.rs) -------------------------------------

#[must_use]
fn json_output_unless_text_selected(args: &[String]) -> bool {
    !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"))
}

fn emit_err(command: &str, message: &str, want_json: bool) -> Result<(), ExitError> {
    let env: CliEnvelope<()> = CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message);
    crate::cli_util::emit_envelope(env, want_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parse_record_requires_conflict_file() {
        let error = parse_record_args(&args(&[])).expect_err("missing required");
        assert_eq!(
            error,
            GovernanceParseError::MissingRequired {
                flag: "conflict-file",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_record_accepts_complete_args() {
        let outcome =
            parse_record_args(&args(&["--conflict-file", "c.yaml", "--no-json"])).expect("parse");
        let GovernanceParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(opts.conflict_file, PathBuf::from("c.yaml"));
        assert!(!opts.common.want_json);
    }

    #[test]
    fn parse_conflicts_accepts_valid_status() {
        let outcome = parse_conflicts_args(&args(&["--status", "pending"])).expect("parse");
        let GovernanceParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(opts.status.as_deref(), Some("pending"));
    }

    #[test]
    fn parse_conflicts_rejects_bad_status() {
        let error = parse_conflicts_args(&args(&["--status", "bogus"])).expect_err("bad status");
        assert!(
            matches!(error, GovernanceParseError::InvalidStatus { .. }),
            "expected InvalidStatus: {error:?}"
        );
    }

    #[test]
    fn parse_arbitrate_requires_conflict_id_policy_arbiter() {
        let error = parse_arbitrate_args(&args(&[])).expect_err("missing");
        assert_eq!(
            error,
            GovernanceParseError::MissingRequired {
                flag: "conflict-id",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_arbitrate_accepts_awarded_to() {
        let outcome = parse_arbitrate_args(&args(&[
            "--conflict-id",
            "c.1",
            "--policy-file",
            "p.yaml",
            "--arbiter",
            "principal.daniel",
            "--awarded-to",
            "principal.alice",
        ]))
        .expect("parse");
        let GovernanceParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(opts.awarded_to.as_deref(), Some("principal.alice"));
        assert!(!opts.both_released);
        assert!(!opts.split_scope);
    }

    #[test]
    fn parse_arbitrate_accepts_both_released_flag() {
        let outcome = parse_arbitrate_args(&args(&[
            "--conflict-id",
            "c.1",
            "--policy-file",
            "p.yaml",
            "--arbiter",
            "principal.daniel",
            "--both-released",
        ]))
        .expect("parse");
        let GovernanceParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert!(opts.both_released);
        assert_eq!(opts.awarded_to, None);
    }

    #[test]
    fn parse_escalate_requires_principal() {
        let error =
            parse_escalate_args(&args(&["--conflict-id", "c.1", "--policy-file", "p.yaml"]))
                .expect_err("missing principal");
        assert_eq!(
            error,
            GovernanceParseError::MissingRequired {
                flag: "principal",
                want_json: true,
            }
        );
    }

    #[test]
    fn run_governance_unknown_subcommand_emits_usage_error() {
        let result = run_governance_command(&args(&["governance", "frobnicate", "--no-json"]));
        assert!(result.is_err(), "unknown subcommand must error");
    }

    #[test]
    fn run_governance_help_prints_usage_and_succeeds() {
        let result = run_governance_command(&args(&["governance", "--help"]));
        assert!(result.is_ok(), "--help must succeed");
    }

    #[test]
    fn governance_usage_projects_command_surface_lines() {
        let mut usage = String::from("forge-core governance <subcommand> [options]");
        for line in COMMAND_GOVERNANCE.local_usage_lines() {
            usage.push('\n');
            usage.push_str("  ");
            usage.push_str(line);
        }

        assert!(
            usage.starts_with("forge-core governance <subcommand> [options]"),
            "governance usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_GOVERNANCE.usage_lines {
            let subcommand_usage = COMMAND_GOVERNANCE.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "governance usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(
            governance_subcommand_hint(),
            "record | conflicts | arbitrate | escalate"
        );
    }

    #[test]
    fn governance_subcommand_help_lookup_projects_full_command_surface_lines() {
        for subcommand in ["record", "conflicts", "arbitrate", "escalate"] {
            let usage = governance_command_surface_usage_line_for(subcommand);
            assert_eq!(
                Some(usage),
                COMMAND_GOVERNANCE.usage_line_for_subcommand(subcommand),
                "governance {subcommand} help should come from the Command Surface"
            );
        }
    }
}
