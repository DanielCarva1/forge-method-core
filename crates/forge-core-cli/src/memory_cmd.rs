//! `forge-core memory` — the CLI surface for the memory trust model (F06.7).
//!
//! Five subcommands wrap the `forge-core-memory` PEP (ADR 0023 + ADR 0024):
//! - `ingest`  — admit a `MemoryEntry` (from a YAML `--entry-file`) under a
//!   `--policy-file`. Calls `forge_core_memory::admit`.
//! - `list`    — lazy TTL sweep + list live entries. Calls `list_now`.
//! - `forget`  — append-only forget with before-image. Calls `forget`.
//! - `promote` — promote an entry's authority with raw evidence. Calls `promote`.
//! - `review`  — DEFERRED. The review axis requires F07 governance (reviewer
//!   authorization) which is not yet implemented; emitting a clear
//!   "not yet implemented" envelope keeps the verb discoverable without a
//!   misleading stub.
//!
//! State writes go to `<state_root>/memory/` (resolved via `resolve_project`,
//! same pattern as `isolation` / `claim`). No claim-governance (`check-write`)
//! is required — state-dir writes are `Ungoverned` by the classifier and gated
//! by the memory file lock instead (confirmed by the CLI integration research).
//!
//! Output: standard [`CliEnvelope`] dual-output (JSON for agents, text for
//! humans), mirroring `autonomy_cmd.rs`.

use std::path::{Path, PathBuf};

use forge_core_command_surface::COMMAND_MEMORY;
use forge_core_contracts::{
    AdmissionEvidence, CliEnvelope, ExitReason, MemoryEntry, MemoryPolicy, StableId,
};
use forge_core_memory::{
    admit, forget, list_now, promote, AdmissionStatus, ForgetStatus, ListStatus, PromoteStatus,
};

use crate::cli_error::ExitError;

const MEMORY_COMMAND: &str = "memory";
const INGEST_COMMAND: &str = "memory ingest";
const LIST_COMMAND: &str = "memory list";
const FORGET_COMMAND: &str = "memory forget";
const PROMOTE_COMMAND: &str = "memory promote";
const REVIEW_COMMAND: &str = "memory review";

/// Parse and run `forge-core memory <subcommand>`.
///
/// # Errors
///
/// Returns `ExitError::usage` (via envelope emission) when the subcommand is
/// unknown or argument parsing fails.
pub fn run_memory_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "ingest" => run_ingest(&args[2..]),
        "list" => run_list(&args[2..]),
        "forget" => run_forget(&args[2..]),
        "promote" => run_promote(&args[2..]),
        "review" => run_review(&args[2..]),
        "--help" | "-h" | "help" => {
            print_memory_usage();
            Ok(())
        }
        other => {
            let want_json = json_output_unless_text_selected(&args[2..]);
            emit_err(
                MEMORY_COMMAND,
                &format!(
                    "unknown subcommand '{other}'. Try: {hint}",
                    hint = memory_subcommand_hint()
                ),
                want_json,
            )
        }
    }
}

fn print_memory_usage() {
    println!("forge-core memory <subcommand> [options]");
    for line in COMMAND_MEMORY.local_usage_lines() {
        println!("  {line}");
    }
    println!();
    println!("  State writes land under <state_root>/memory/ (resolved from --root). --memory-dir overrides the directory.");
}

fn memory_subcommand_hint() -> String {
    COMMAND_MEMORY.concrete_subcommand_hint()
}

fn memory_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_MEMORY
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core memory <subcommand> [options]")
}

fn memory_parse_error_message_with_usage(subcommand: &str, error: &MemoryParseError) -> String {
    format!(
        "{}\n\nusage:\n  {}",
        error.message(),
        memory_command_surface_usage_line_for(subcommand)
    )
}

// --- shared resolution -------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryResolveError {
    message: String,
}

impl MemoryResolveError {
    #[must_use]
    fn message(&self) -> String {
        self.message.clone()
    }
}

/// Resolve the memory state directory: either an explicit `--memory-dir`
/// override, or `<state_root>/memory` resolved from `--root`. Mirrors the
/// `resolve_isolation_dir_or_err` pattern in `isolation.rs`.
fn resolve_memory_dir(
    root: Option<&str>,
    allow_bootstrap_core: bool,
    memory_dir: Option<&str>,
) -> Result<PathBuf, MemoryResolveError> {
    if let Some(dir) = memory_dir {
        let path = PathBuf::from(dir);
        // Create the dir if it doesn't exist (the PEP will create the log file
        // under it; the store's append helper creates parents, but the dir
        // itself is the contract for "this is where memory lives").
        std::fs::create_dir_all(&path).map_err(|source| MemoryResolveError {
            message: format!("cannot create --memory-dir '{}': {source}", path.display()),
        })?;
        return Ok(path);
    }
    let root_str = root.unwrap_or(".");
    let root_path = PathBuf::from(root_str);
    let project = crate::project_cmd::resolve_project(&root_path, allow_bootstrap_core).map_err(
        |source| MemoryResolveError {
            message: format!("cannot resolve Forge project from --root '{root_str}': {source}"),
        },
    )?;
    let state_root = PathBuf::from(&project.state_root);
    if !state_root.is_dir() {
        return Err(MemoryResolveError {
            message: format!(
                "resolved Forge state_root is not a directory: {}; create the sidecar .forge-method directory or pass --memory-dir",
                state_root.display()
            ),
        });
    }
    let memory_root = state_root.join("memory");
    std::fs::create_dir_all(&memory_root).map_err(|source| MemoryResolveError {
        message: format!(
            "cannot create memory dir '{}': {source}",
            memory_root.display()
        ),
    })?;
    Ok(memory_root)
}

// --- common option fields ----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CommonOptions {
    root: Option<String>,
    allow_bootstrap_core: bool,
    memory_dir: Option<String>,
    want_json: bool,
}

/// Parse the common `--root` / `--allow-bootstrap-core` / `--memory-dir` /
/// `--no-json` / `--text` flags into `common`. Returns `Some(unknown_arg)` if
/// the flag is not one of these (so the caller can handle subcommand-specific
/// flags); returns `None` when the flag was consumed.
enum CommonFlag {
    Consumed,
    Unknown(String),
}

fn parse_common_flag(
    args: &[String],
    idx: &mut usize,
    common: &mut CommonOptions,
) -> Result<CommonFlag, MemoryParseError> {
    // `want_json` is snapshotted here so the borrow of `common` for the field
    // assignments below does not conflict with reading it for error construction.
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
        "--memory-dir" => {
            *idx += 1;
            let value = require_value(args, *idx, "memory-dir", want_json)?;
            common.memory_dir = Some(value);
            Ok(CommonFlag::Consumed)
        }
        "--no-json" | "--text" => {
            common.want_json = false;
            Ok(CommonFlag::Consumed)
        }
        // `--json` is the default (want_json starts true); accepting it as a
        // no-op keeps the flag documented in --help usable.
        "--json" => Ok(CommonFlag::Consumed),
        other => Ok(CommonFlag::Unknown(other.to_string())),
    }
}

// --- ingest ------------------------------------------------------------------

fn run_ingest(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_ingest_args(args) {
        Ok(MemoryParseOutcome::Help) => {
            println!("{}", memory_command_surface_usage_line_for("ingest"));
            return Ok(());
        }
        Ok(MemoryParseOutcome::Run(opts)) => opts,
        Err(error) => {
            return emit_err(
                INGEST_COMMAND,
                &memory_parse_error_message_with_usage("ingest", &error),
                error.want_json(),
            );
        }
    };
    let entry = match load_entry_file(&outcome.entry_file) {
        Ok(entry) => entry,
        Err(error) => return emit_err(INGEST_COMMAND, &error.message(), outcome.common.want_json),
    };
    let policy = match load_policy_file(&outcome.policy_file) {
        Ok(policy) => policy,
        Err(error) => return emit_err(INGEST_COMMAND, &error.message(), outcome.common.want_json),
    };
    let memory_dir = match resolve_memory_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.memory_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return emit_err(INGEST_COMMAND, &error.message(), outcome.common.want_json),
    };

    let result = admit(&memory_dir, entry, &policy);
    let env: CliEnvelope<serde_json::Value> = match result.status {
        AdmissionStatus::Admitted { sequence } => CliEnvelope::ok(
            INGEST_COMMAND,
            serde_json::to_value(IngestOkData {
                entry_id: result.entry_id.0,
                sequence,
            })
            .expect("serialize ingest ok"),
        ),
        AdmissionStatus::DeniedByGate(reasons) => CliEnvelope::reject(
            INGEST_COMMAND,
            ExitReason::RejectedByGate,
            "admission gate denied the entry",
            serde_json::to_value(reasons_iter(reasons)).expect("serialize denial reasons"),
        ),
        AdmissionStatus::StoreError(error) => CliEnvelope::err(
            INGEST_COMMAND,
            ExitReason::Conflict,
            format!("memory store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct IngestOkData {
    entry_id: String,
    sequence: u64,
}

// --- list --------------------------------------------------------------------

fn run_list(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_list_args(args) {
        Ok(MemoryParseOutcome::Help) => {
            println!("{}", memory_command_surface_usage_line_for("list"));
            return Ok(());
        }
        Ok(MemoryParseOutcome::Run(opts)) => opts,
        Err(error) => {
            return emit_err(
                LIST_COMMAND,
                &memory_parse_error_message_with_usage("list", &error),
                error.want_json(),
            );
        }
    };
    let memory_dir = match resolve_memory_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.memory_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return emit_err(LIST_COMMAND, &error.message(), outcome.common.want_json),
    };
    let now_unix = outcome.now_unix.unwrap_or_else(forge_core_memory::now_unix);
    let result = list_now(&memory_dir, now_unix);
    let env = match result.status {
        ListStatus::Ok { flipped, entries } => CliEnvelope::ok(
            LIST_COMMAND,
            ListOkData {
                flipped,
                now_unix,
                entries: entries.into_iter().map(list_entry_view).collect(),
            },
        ),
        ListStatus::StoreError(error) => CliEnvelope::err(
            LIST_COMMAND,
            ExitReason::Conflict,
            format!("memory store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ListOkData {
    flipped: usize,
    now_unix: u64,
    entries: Vec<ListEntryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ListEntryView {
    entry_id: String,
    kind: String,
    authority: String,
    review: String,
    stale: bool,
}

fn list_entry_view(entry: MemoryEntry) -> ListEntryView {
    let entry_id = entry.entry_id.0.clone();
    let kind = format!("{:?}", entry.kind).to_lowercase();
    let authority = format!("{:?}", entry.authority_level_effective()).to_lowercase();
    let review = format!("{:?}", entry.review_state_effective()).to_lowercase();
    let stale = entry.freshness.stale;
    ListEntryView {
        entry_id,
        kind,
        authority,
        review,
        stale,
    }
}

// --- forget ------------------------------------------------------------------

fn run_forget(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_forget_args(args) {
        Ok(MemoryParseOutcome::Help) => {
            println!("{}", memory_command_surface_usage_line_for("forget"));
            return Ok(());
        }
        Ok(MemoryParseOutcome::Run(opts)) => opts,
        Err(error) => {
            return emit_err(
                FORGET_COMMAND,
                &memory_parse_error_message_with_usage("forget", &error),
                error.want_json(),
            );
        }
    };
    let memory_dir = match resolve_memory_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.memory_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return emit_err(FORGET_COMMAND, &error.message(), outcome.common.want_json),
    };
    let result = forget(&memory_dir, StableId(outcome.entry_id.clone()));
    let env = match result.status {
        ForgetStatus::Forgotten { sequence } => CliEnvelope::ok(
            FORGET_COMMAND,
            ForgetOkData {
                entry_id: result.entry_id.0,
                sequence,
            },
        ),
        ForgetStatus::AlreadyForgotten => CliEnvelope::ok(
            FORGET_COMMAND,
            ForgetOkData {
                entry_id: result.entry_id.0,
                sequence: 0,
            },
        ),
        ForgetStatus::NotFound => CliEnvelope::err(
            FORGET_COMMAND,
            ExitReason::RejectedByGate,
            format!(
                "entry '{}' not found (never admitted or already forgotten)",
                outcome.entry_id
            ),
        ),
        ForgetStatus::StoreError(error) => CliEnvelope::err(
            FORGET_COMMAND,
            ExitReason::Conflict,
            format!("memory store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct ForgetOkData {
    entry_id: String,
    sequence: u64,
}

// --- promote -----------------------------------------------------------------

fn run_promote(args: &[String]) -> Result<(), ExitError> {
    let outcome = match parse_promote_args(args) {
        Ok(MemoryParseOutcome::Help) => {
            println!("{}", memory_command_surface_usage_line_for("promote"));
            return Ok(());
        }
        Ok(MemoryParseOutcome::Run(opts)) => opts,
        Err(error) => {
            return emit_err(
                PROMOTE_COMMAND,
                &memory_parse_error_message_with_usage("promote", &error),
                error.want_json(),
            );
        }
    };
    let policy = match load_policy_file(&outcome.policy_file) {
        Ok(policy) => policy,
        Err(error) => return emit_err(PROMOTE_COMMAND, &error.message(), outcome.common.want_json),
    };
    let memory_dir = match resolve_memory_dir(
        outcome.common.root.as_deref(),
        outcome.common.allow_bootstrap_core,
        outcome.common.memory_dir.as_deref(),
    ) {
        Ok(dir) => dir,
        Err(error) => return emit_err(PROMOTE_COMMAND, &error.message(), outcome.common.want_json),
    };
    let evidence = AdmissionEvidence {
        evidence_refs: outcome.evidence_refs,
    };
    let result = promote(
        &memory_dir,
        StableId(outcome.entry_id.clone()),
        &policy,
        &evidence,
    );
    let env: CliEnvelope<serde_json::Value> = match result.status {
        PromoteStatus::Promoted {
            sequence,
            before,
            after,
        } => CliEnvelope::ok(
            PROMOTE_COMMAND,
            serde_json::to_value(PromoteOkData {
                entry_id: result.entry_id.0,
                sequence,
                before: format!("{before:?}").to_lowercase(),
                after: format!("{after:?}").to_lowercase(),
            })
            .expect("serialize promote ok"),
        ),
        PromoteStatus::NotFound => CliEnvelope::err(
            PROMOTE_COMMAND,
            ExitReason::RejectedByGate,
            format!("entry '{}' not found; cannot promote", outcome.entry_id),
        ),
        PromoteStatus::DeniedByGate(reasons) => CliEnvelope::reject(
            PROMOTE_COMMAND,
            ExitReason::RejectedByGate,
            "promote gate denied (insufficient raw evidence)",
            serde_json::to_value(reasons_iter(reasons)).expect("serialize denial reasons"),
        ),
        PromoteStatus::StoreError(error) => CliEnvelope::err(
            PROMOTE_COMMAND,
            ExitReason::Conflict,
            format!("memory store error: {error}"),
        ),
    };
    crate::cli_util::emit_envelope(env, outcome.common.want_json)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct PromoteOkData {
    entry_id: String,
    sequence: u64,
    before: String,
    after: String,
}

// --- review (deferred) -------------------------------------------------------

fn run_review(args: &[String]) -> Result<(), ExitError> {
    // The review axis requires F07 governance (reviewer authorization via a
    // GovernancePolicy) which is not yet implemented. Rather than ship a stub
    // that silently flips a boolean, we emit a clear "deferred" envelope so
    // the verb is discoverable but cannot mislead a caller into thinking a
    // review landed. This is the ADR-0023 orthogonality discipline: review is
    // a principal attestation, not a magic boolean.
    let want_json = json_output_unless_text_selected(args);
    if let Err(error) = parse_common_only(args) {
        return emit_err(
            REVIEW_COMMAND,
            &memory_parse_error_message_with_usage("review", &error),
            error.want_json(),
        );
    }
    emit_err(
        REVIEW_COMMAND,
        "review is deferred: the review axis requires F07 governance (reviewer authorization), which is not yet implemented",
        want_json,
    )
}

// --- arg parsing (per subcommand) --------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum MemoryParseOutcome<T> {
    Help,
    Run(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MemoryParseError {
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
    InvalidU64 {
        flag: &'static str,
        value: String,
        want_json: bool,
    },
    UnknownArgument {
        argument: String,
        want_json: bool,
    },
}

impl MemoryParseError {
    #[must_use]
    fn want_json(&self) -> bool {
        match self {
            Self::MissingValue { want_json, .. }
            | Self::FlagAsValue { want_json, .. }
            | Self::MissingRequired { want_json, .. }
            | Self::InvalidU64 { want_json, .. }
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
            Self::InvalidU64 { flag, value, .. } => {
                format!("--{flag} must be a non-negative integer, got '{value}'")
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
) -> Result<String, MemoryParseError> {
    match args.get(idx) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(MemoryParseError::FlagAsValue {
                flag,
                value: value.clone(),
                want_json,
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(MemoryParseError::MissingValue { flag, want_json }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IngestOptions {
    common: CommonOptions,
    entry_file: PathBuf,
    policy_file: PathBuf,
}

fn parse_ingest_args(
    args: &[String],
) -> Result<MemoryParseOutcome<IngestOptions>, MemoryParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut entry_file: Option<PathBuf> = None;
    let mut policy_file: Option<PathBuf> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--entry-file" => {
                    idx += 1;
                    entry_file = Some(PathBuf::from(require_value(
                        args,
                        idx,
                        "entry-file",
                        common.want_json,
                    )?));
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
                "--help" | "-h" => return Ok(MemoryParseOutcome::Help),
                other => {
                    return Err(MemoryParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let entry_file = entry_file.ok_or(MemoryParseError::MissingRequired {
        flag: "entry-file",
        want_json: common.want_json,
    })?;
    let policy_file = policy_file.ok_or(MemoryParseError::MissingRequired {
        flag: "policy-file",
        want_json: common.want_json,
    })?;
    Ok(MemoryParseOutcome::Run(IngestOptions {
        common,
        entry_file,
        policy_file,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListOptions {
    common: CommonOptions,
    now_unix: Option<u64>,
}

fn parse_list_args(args: &[String]) -> Result<MemoryParseOutcome<ListOptions>, MemoryParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut now_unix: Option<u64> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--now-unix" => {
                    idx += 1;
                    let raw = require_value(args, idx, "now-unix", common.want_json)?;
                    now_unix =
                        Some(
                            raw.parse::<u64>()
                                .map_err(|_| MemoryParseError::InvalidU64 {
                                    flag: "now-unix",
                                    value: raw,
                                    want_json: common.want_json,
                                })?,
                        );
                }
                "--help" | "-h" => return Ok(MemoryParseOutcome::Help),
                other => {
                    return Err(MemoryParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    Ok(MemoryParseOutcome::Run(ListOptions { common, now_unix }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForgetOptions {
    common: CommonOptions,
    entry_id: String,
}

fn parse_forget_args(
    args: &[String],
) -> Result<MemoryParseOutcome<ForgetOptions>, MemoryParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut entry_id: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--entry-id" => {
                    idx += 1;
                    entry_id = Some(require_value(args, idx, "entry-id", common.want_json)?);
                }
                "--help" | "-h" => return Ok(MemoryParseOutcome::Help),
                other => {
                    return Err(MemoryParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let entry_id = entry_id.ok_or(MemoryParseError::MissingRequired {
        flag: "entry-id",
        want_json: common.want_json,
    })?;
    Ok(MemoryParseOutcome::Run(ForgetOptions { common, entry_id }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromoteOptions {
    common: CommonOptions,
    entry_id: String,
    policy_file: PathBuf,
    evidence_refs: Vec<String>,
}

fn parse_promote_args(
    args: &[String],
) -> Result<MemoryParseOutcome<PromoteOptions>, MemoryParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut entry_id: Option<String> = None;
    let mut policy_file: Option<PathBuf> = None;
    let mut evidence_refs: Vec<String> = Vec::new();
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--entry-id" => {
                    idx += 1;
                    entry_id = Some(require_value(args, idx, "entry-id", common.want_json)?);
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
                "--evidence" => {
                    idx += 1;
                    evidence_refs.push(require_value(args, idx, "evidence", common.want_json)?);
                }
                "--help" | "-h" => return Ok(MemoryParseOutcome::Help),
                other => {
                    return Err(MemoryParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    let entry_id = entry_id.ok_or(MemoryParseError::MissingRequired {
        flag: "entry-id",
        want_json: common.want_json,
    })?;
    let policy_file = policy_file.ok_or(MemoryParseError::MissingRequired {
        flag: "policy-file",
        want_json: common.want_json,
    })?;
    Ok(MemoryParseOutcome::Run(PromoteOptions {
        common,
        entry_id,
        policy_file,
        evidence_refs,
    }))
}

/// Parse only the common flags (for `review`, which validates flags but does
/// not act). Returns the resolved `CommonOptions` or an error.
fn parse_common_only(args: &[String]) -> Result<CommonOptions, MemoryParseError> {
    let mut common = CommonOptions {
        want_json: true,
        ..CommonOptions::default()
    };
    let mut idx = 0usize;
    while idx < args.len() {
        match parse_common_flag(args, &mut idx, &mut common)? {
            CommonFlag::Consumed => {}
            CommonFlag::Unknown(flag) => match flag.as_str() {
                "--help" | "-h" => return Ok(common),
                other => {
                    return Err(MemoryParseError::UnknownArgument {
                        argument: other.to_string(),
                        want_json: common.want_json,
                    });
                }
            },
        }
        idx += 1;
    }
    Ok(common)
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

fn load_entry_file(path: &Path) -> Result<MemoryEntry, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError {
        message: format!("cannot read entry file '{}': {source}", path.display()),
    })?;
    yaml_serde::from_str::<MemoryEntry>(&text).map_err(|source| LoadError {
        message: format!(
            "entry file '{}' is not a valid MemoryEntry YAML: {source}",
            path.display()
        ),
    })
}

fn load_policy_file(path: &Path) -> Result<MemoryPolicy, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError {
        message: format!("cannot read policy file '{}': {source}", path.display()),
    })?;
    yaml_serde::from_str::<MemoryPolicy>(&text).map_err(|source| LoadError {
        message: format!(
            "policy file '{}' is not a valid MemoryPolicy YAML: {source}",
            path.display()
        ),
    })
}

// --- emit helpers (mirror autonomy_cmd.rs) -----------------------------------

#[must_use]
fn json_output_unless_text_selected(args: &[String]) -> bool {
    !args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--no-json" | "--text"))
}

/// Convert a `Vec<AdmissionDenialReason>` into a serializable list of strings
/// for the envelope's `data` on a rejection. Uses serde's `rename_all =
/// "snake_case"` (not `Debug`, which has no underscores) so the wire form
/// matches the YAML/JSON contract form.
#[must_use]
fn reasons_iter(reasons: Vec<forge_core_contracts::AdmissionDenialReason>) -> Vec<String> {
    reasons
        .into_iter()
        .map(|r| {
            serde_json::to_value(r)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| format!("{r:?}").to_lowercase())
        })
        .collect()
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

    fn assert_memory_error_projects_only_subcommand_usage(
        error: &MemoryParseError,
        subcommand: &str,
        expected_diagnostic: &str,
    ) {
        let message = memory_parse_error_message_with_usage(subcommand, error);
        assert!(
            message.contains(expected_diagnostic),
            "error should preserve diagnostic {expected_diagnostic:?}: {message}"
        );
        let projected = COMMAND_MEMORY
            .usage_line_for_subcommand(subcommand)
            .expect("memory subcommand usage");
        assert!(
            message.contains(projected),
            "error should project {subcommand} Command Surface usage {projected:?}: {message}"
        );
        for sibling in ["ingest", "list", "forget", "promote", "review"] {
            if sibling != subcommand {
                let sibling_usage = COMMAND_MEMORY
                    .usage_line_for_subcommand(sibling)
                    .expect("sibling usage");
                assert!(
                    !message.contains(sibling_usage),
                    "error for {subcommand} should not leak {sibling} usage: {message}"
                );
            }
        }
    }

    #[test]
    fn require_value_returns_present_value() {
        let a = args(&["--x", "v"]);
        let v = require_value(&a, 1, "x", true).expect("value");
        assert_eq!(v, "v");
    }

    #[test]
    fn require_value_rejects_missing_value() {
        let a = args(&["--entry-id"]);
        let error = require_value(&a, 1, "entry-id", true).expect_err("missing");
        assert_eq!(
            error,
            MemoryParseError::MissingValue {
                flag: "entry-id",
                want_json: true,
            }
        );
    }

    #[test]
    fn require_value_rejects_flag_as_value() {
        let a = args(&["--entry-id", "--root"]);
        let error = require_value(&a, 1, "entry-id", true).expect_err("flag-as-value");
        assert_eq!(
            error,
            MemoryParseError::FlagAsValue {
                flag: "entry-id",
                value: "--root".to_string(),
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_ingest_requires_entry_and_policy_file() {
        let error = parse_ingest_args(&args(&[])).expect_err("missing required");
        assert_eq!(
            error,
            MemoryParseError::MissingRequired {
                flag: "entry-file",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_ingest_accepts_complete_args() {
        let outcome = parse_ingest_args(&args(&[
            "--entry-file",
            "entry.yaml",
            "--policy-file",
            "policy.yaml",
            "--no-json",
        ]))
        .expect("parse");
        let MemoryParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(opts.entry_file, PathBuf::from("entry.yaml"));
        assert_eq!(opts.policy_file, PathBuf::from("policy.yaml"));
        assert!(!opts.common.want_json);
    }

    #[test]
    fn parse_ingest_respects_text_preference_on_error() {
        let error = parse_ingest_args(&args(&["--no-json", "--unknown"])).expect_err("unknown");
        assert!(
            matches!(
                error,
                MemoryParseError::UnknownArgument {
                    want_json: false,
                    ..
                }
            ),
            "want_json must be false: {error:?}"
        );
    }

    #[test]
    fn parse_forget_requires_entry_id() {
        let error = parse_forget_args(&args(&[])).expect_err("missing");
        assert_eq!(
            error,
            MemoryParseError::MissingRequired {
                flag: "entry-id",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_promote_requires_entry_id_policy_and_evidence() {
        // Missing all three.
        let error = parse_promote_args(&args(&[])).expect_err("missing");
        assert_eq!(
            error,
            MemoryParseError::MissingRequired {
                flag: "entry-id",
                want_json: true,
            }
        );
        // entry-id present, policy-file missing.
        let error = parse_promote_args(&args(&["--entry-id", "e.one"])).expect_err("missing");
        assert_eq!(
            error,
            MemoryParseError::MissingRequired {
                flag: "policy-file",
                want_json: true,
            }
        );
    }

    #[test]
    fn parse_promote_accepts_multiple_evidence_refs() {
        let outcome = parse_promote_args(&args(&[
            "--entry-id",
            "e.one",
            "--policy-file",
            "p.yaml",
            "--evidence",
            "run.alpha",
            "--evidence",
            "run.beta",
        ]))
        .expect("parse");
        let MemoryParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(
            opts.evidence_refs,
            vec!["run.alpha".to_string(), "run.beta".to_string()]
        );
    }

    #[test]
    fn parse_list_accepts_now_unix() {
        let outcome = parse_list_args(&args(&["--now-unix", "1700000000"])).expect("parse");
        let MemoryParseOutcome::Run(opts) = outcome else {
            panic!("expected Run");
        };
        assert_eq!(opts.now_unix, Some(1_700_000_000));
    }

    #[test]
    fn parse_list_rejects_bad_now_unix() {
        let error = parse_list_args(&args(&["--now-unix", "not-a-number"])).expect_err("bad u64");
        assert!(
            matches!(
                error,
                MemoryParseError::InvalidU64 {
                    flag: "now-unix",
                    ..
                }
            ),
            "expected InvalidU64: {error:?}"
        );
    }

    #[test]
    fn parse_common_only_rejects_unknown_flag() {
        let error = parse_common_only(&args(&["--bogus"])).expect_err("unknown");
        assert!(matches!(error, MemoryParseError::UnknownArgument { .. }));
    }

    #[test]
    fn json_output_unless_text_selected_defaults_to_json() {
        assert!(json_output_unless_text_selected(&args(&[])));
        assert!(!json_output_unless_text_selected(&args(&["--no-json"])));
        assert!(!json_output_unless_text_selected(&args(&["--text"])));
    }

    #[test]
    fn reasons_iter_lowercases_variants() {
        let reasons = vec![
            forge_core_contracts::AdmissionDenialReason::KindNotPermitted,
            forge_core_contracts::AdmissionDenialReason::InsufficientEvidenceForAuthority,
        ];
        let out = reasons_iter(reasons);
        assert_eq!(
            out,
            vec!["kind_not_permitted", "insufficient_evidence_for_authority"]
        );
    }

    #[test]
    fn run_review_emits_deferred_envelope() {
        // review is deferred — it must emit an error envelope (non-zero exit)
        // regardless of flags, and never panic.
        let result = run_review(&args(&["--no-json"]));
        assert!(
            result.is_err(),
            "deferred review must return a non-zero ExitError"
        );
    }

    #[test]
    fn run_memory_unknown_subcommand_emits_usage_error() {
        let result = run_memory_command(&args(&["memory", "frobnicate", "--no-json"]));
        assert!(result.is_err(), "unknown subcommand must error");
    }

    #[test]
    fn run_memory_help_prints_usage_and_succeeds() {
        let result = run_memory_command(&args(&["memory", "--help"]));
        assert!(result.is_ok(), "--help must succeed");
    }

    #[test]
    fn memory_usage_projects_command_surface_lines() {
        let mut usage = String::from("forge-core memory <subcommand> [options]");
        for line in COMMAND_MEMORY.local_usage_lines() {
            usage.push('\n');
            usage.push_str("  ");
            usage.push_str(line);
        }

        assert!(
            usage.starts_with("forge-core memory <subcommand> [options]"),
            "memory usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_MEMORY.usage_lines {
            let subcommand_usage = COMMAND_MEMORY.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "memory usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(
            memory_subcommand_hint(),
            "ingest | list | forget | promote | review"
        );
    }

    #[test]
    fn memory_subcommand_help_lookup_projects_full_command_surface_lines() {
        for subcommand in ["ingest", "list", "forget", "promote", "review"] {
            let usage = memory_command_surface_usage_line_for(subcommand);
            assert_eq!(
                Some(usage),
                COMMAND_MEMORY.usage_line_for_subcommand(subcommand),
                "memory {subcommand} help should come from the Command Surface"
            );
        }
    }

    #[test]
    fn memory_missing_value_reports_subcommand_usage() {
        let error = parse_ingest_args(&args(&["--entry-file", "--policy-file"]))
            .expect_err("missing value");

        assert_memory_error_projects_only_subcommand_usage(
            &error,
            "ingest",
            "--entry-file requires a value, got another flag '--policy-file'",
        );
    }

    #[test]
    fn memory_unknown_arg_reports_subcommand_usage() {
        let error = parse_list_args(&args(&["--bogus"])).expect_err("unknown arg");

        assert_memory_error_projects_only_subcommand_usage(
            &error,
            "list",
            "unknown argument '--bogus'",
        );
    }

    #[test]
    fn memory_missing_required_reports_subcommand_usage() {
        let forget_error = parse_forget_args(&args(&[])).expect_err("missing entry");
        assert_memory_error_projects_only_subcommand_usage(
            &forget_error,
            "forget",
            "--entry-id is required",
        );

        let promote_error =
            parse_promote_args(&args(&["--entry-id", "e.one"])).expect_err("missing policy");
        assert_memory_error_projects_only_subcommand_usage(
            &promote_error,
            "promote",
            "--policy-file is required",
        );
    }

    #[test]
    fn memory_invalid_value_reports_subcommand_usage() {
        let error =
            parse_list_args(&args(&["--now-unix", "not-a-number"])).expect_err("invalid now");

        assert_memory_error_projects_only_subcommand_usage(
            &error,
            "list",
            "--now-unix must be a non-negative integer, got 'not-a-number'",
        );
    }

    #[test]
    fn memory_review_flag_errors_project_review_usage() {
        let error = parse_common_only(&args(&["--bogus"])).expect_err("unknown review flag");

        assert_memory_error_projects_only_subcommand_usage(
            &error,
            "review",
            "unknown argument '--bogus'",
        );
    }
}
