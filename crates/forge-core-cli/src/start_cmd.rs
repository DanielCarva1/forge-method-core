//! `forge-core start` — F12 Guided Start and zero-config bootstrap.
//!
//! `start` advances a Consumer Project Repo from an empty state to the point
//! where agent-native workflow governance can take over. It inspects the real
//! project state, creates the canonical Project Link/sidecar only when no link
//! exists, preserves one of five compatibility [`BootstrapState`] wire values,
//! and emits a structured next step. A linked missing or incomplete sidecar is
//! possible durable-state loss and fails closed without mutation.
//!
//! ## Authority boundary
//!
//! Bootstrap mutation is confined to first initialization through `project init`.
//! Once a Project Link exists, `start` never recreates linked state. It does not
//! choose a workflow, phase, bundle, target, evidence result, or completion.
//!
//! ## State machine
//!
//! The five states are documented as domain terms. Each maps to one outcome:
//!
//! | state                         | outcome                                    |
//! |-------------------------------|--------------------------------------------|
//! | `no_link`                     | bootstrap, then `workflow init --root …`   |
//! | `link_present_no_sidecar`     | fail closed; inspect/restore durable state |
//! | `sidecar_ready_no_contract`   | `workflow init --root …`                   |
//! | `contract_present`            | `workflow init --root …`                   |
//! | `preview_run`                 | `workflow init --root …`                   |
//!
//! `start` recomputes from the real project on every call. Re-running after a
//! successful advance jumps to the correct state; re-running after state loss
//! remains nonmutating until an explicit recovery flow exists.
//!
//! ## Anti-script-de-novela (G1)
//!
//! `start` is parametric, not prescriptive: it adapts its output to the
//! project's real state and never dictates spec content or lets compatibility
//! prose select governance. The `sidecar_ready_no_contract` payload retains
//! canonical operation scenarios and validation commands as secondary
//! references. The deletion test holds: state detection plus safe bootstrap
//! and structured handoff is non-trivial behaviour, not a linear
//! script.

use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

use forge_core_command_surface::COMMAND_START;
use forge_core_contracts::{
    BootstrapRecoveryChoices, BootstrapStateLossDiagnostic, CliEnvelope, ExitReason,
    BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION, ENVELOPE_SCHEMA_VERSION, PROJECT_LINK_SCHEMA_VERSION,
};
pub use forge_core_contracts::{StateLossCause, StateLossKind, StateLossReleaseStatus};

use crate::cli_error::ExitError;
use crate::project_cmd::{
    init_project, linked_state_loss_detail, resolve_project, resolve_project_observed,
    write_initial_project_state, ProjectInitError, ProjectInitStatus, ProjectLayoutKind,
    ProjectResolveError, ProjectResolvePayload,
};

/// Usage line for `forge-core start`, projected from the shared Command Surface.
///
/// `start` used to keep a second hand-written usage constant here while the
/// global registry projected help from `forge-core-command-surface`. Keeping
/// this as a function instead of a rival constant preserves the parser/help
/// seam: one shared command fact, rendered by both global help and local help.
#[must_use]
pub fn start_usage_line() -> &'static str {
    COMMAND_START.canonical_usage().trim_start()
}

/// Canonical reference scenarios shown in the `sidecar_ready_no_contract`
/// state. These are structural references, not templates to copy verbatim:
/// the agent (with the human) decides what to specify.
const OPERATION_CONTRACT_REFERENCE_DIR: &str = "docs/fixtures/operation-contract-v0/";

/// The command that validates an authored operation contract. `start` points
/// at it but does not run it; the agent runs it once a spec exists.
const OPERATION_VALIDATION_COMMAND: &str = "forge-core preview --operation <path>";

/// Two fixtures hand-picked as the smallest structurally-complete models for
/// a *first* operation contract. `start` surfaces these by name (not the whole
/// directory) so a new agent has an obvious starting point instead of 23
/// undifferentiated files. They are references, not prescriptions: the agent
/// adapts the structure to the human's actual goal.
///
/// - `observe-project-status` — the simplest shape: a read-only operation
///   (`autonomy.mode: observe`). Best model when the first step is to look at
///   the project rather than change it.
/// - `execute-trivial-write` — the simplest *write* shape: shows the
///   `authority.mutation_policy` field. Best model when the first step is a
///   small, reversible change.
const STARTER_FIXTURE_OBSERVE: &str = "observe-project-status.yaml";
const STARTER_FIXTURE_EXECUTE: &str = "execute-trivial-write.yaml";

/// Typed adapter output for `forge-core start` argv parsing.
///
/// Keeping this as a named module interface gives tests and future adapters a
/// seam that is smaller than the full command handler: parse once into stable
/// options, then run the read-only diagnostic core.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StartCliOptions {
    root: PathBuf,
    agent_id: Option<String>,
    output: StartOutputMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartOutputMode {
    Json,
    Text,
}

impl StartOutputMode {
    const fn wants_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StartParseOutcome {
    Run(StartCliOptions),
    Help,
}

/// Argument errors emitted by the typed `start` parser adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StartParseError {
    MissingValue { flag: &'static str },
    FlagAsValue { flag: &'static str, value: String },
    UnknownArgument { argument: String },
}

impl std::fmt::Display for StartParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingValue { flag } => write!(formatter, "start: {flag} requires a value"),
            Self::FlagAsValue { flag, value } => write!(
                formatter,
                "start: {flag} requires a value, got another flag '{value}'"
            ),
            Self::UnknownArgument { argument } => {
                write!(formatter, "start: unrecognized argument '{argument}'")
            }
        }
    }
}

impl std::error::Error for StartParseError {}

fn require_start_value(
    args: &[String],
    index: usize,
    flag: &'static str,
) -> Result<String, StartParseError> {
    match args.get(index) {
        Some(value) if value.starts_with('-') && value.len() > 1 => {
            Err(StartParseError::FlagAsValue {
                flag,
                value: value.clone(),
            })
        }
        Some(value) => Ok(value.clone()),
        None => Err(StartParseError::MissingValue { flag }),
    }
}

fn start_parse_error_with_usage(error: &StartParseError) -> String {
    format!("{}\n\nusage:\n  {}", error, start_usage_line())
}

/// The structured `start` payload. Successful bootstrap/routing carries it as
/// normal data; linked state loss carries it as rejection data so agents can
/// branch on [`BootstrapState`] without parsing diagnostics.
///
/// Only `Serialize` is derived: this is an output payload emitted to stdout,
/// never deserialized back (mirrors `ProjectResolvePayload` / `StatusPayload`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StartPayload {
    /// Envelope contract version (matches the rest of the CLI).
    pub schema_version: String,
    /// Optional host-supplied agent identifier. This field exists so host
    /// surfaces can correlate diagnostics without changing the bootstrap state
    /// machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// Where the project is on the bootstrap path. See [`BootstrapState`].
    pub state: BootstrapState,
    /// Human-legible explanation of why this state was diagnosed, so the
    /// agent can render it without re-deriving the logic.
    pub reason: String,
    /// The concrete next action. `None` only for the terminal `preview_run`
    /// state, where the project is onboarded and `start` has nothing left to
    /// recommend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_step: Option<NextStep>,
    /// Resolved project context. Present once a Project Link exists (i.e. for
    /// every state except `no_link`). Absent for `no_link` because there is
    /// nothing valid to resolve yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectContext>,
    /// Typed identity/status projection present only for linked state loss.
    /// It intentionally carries no secret or host-keystore paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_loss: Option<StateLossStatus>,
    /// Bootstrap actions `start` performed on this call (currently only
    /// `["initialized"]`). Empty when the call is read-only or rejected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions_performed: Vec<String>,
}

/// Compatibility alias retained for Rust callers of the original `start` API.
/// The nested wire contract is now independently versioned.
pub type StateLossStatus = BootstrapStateLossDiagnostic;

/// The five bootstrap states a Consumer Project Repo can be in along the path
/// `start` diagnoses.
///
/// Wire form is `snake_case` to match the rest of the CLI contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapState {
    /// No Project Link present.
    NoLink,
    /// Link exists but linked sidecar/state authority is unavailable or incomplete.
    LinkPresentNoSidecar,
    /// State tree is healthy but no operation spec exists yet.
    SidecarReadyNoContract,
    /// At least one operation spec exists.
    ContractPresent,
    /// A preview has already been produced.
    PreviewRun,
}

impl BootstrapState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoLink => "no_link",
            Self::LinkPresentNoSidecar => "link_present_no_sidecar",
            Self::SidecarReadyNoContract => "sidecar_ready_no_contract",
            Self::ContractPresent => "contract_present",
            Self::PreviewRun => "preview_run",
        }
    }
}

/// A concrete next action `start` recommends. The agent executes it (or
/// surfaces it to the human); `start` itself never acts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NextStep {
    /// A copy-pasteable command string for humans, when one applies. This is
    /// display-only; agents and hosts should prefer [`Self::argv`] so paths
    /// with spaces are not re-tokenized through a shell.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Typed argv for agents/hosts to execute directly. The first entry is the
    /// executable and each following entry is one already-tokenized argument.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    /// One short line describing the step in agent-readable prose.
    pub description: String,
    /// Additional pointers the agent may need (paths, reference dirs). Flat
    /// list of strings so the payload stays forward-compatible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
}

/// The resolved project context, mirrored from `project resolve`. Included
/// verbatim so the agent has everything without a second call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProjectContext {
    pub project_id: String,
    pub project_root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_path: Option<String>,
    pub sidecar_root: String,
    pub state_root: String,
    pub state_exists: bool,
    pub layout: ProjectLayoutKind,
    /// Bootstrap compatibility phase read from `state.yaml`. P5 workflow
    /// governance derives its phase from the workflow ledger instead. `None`
    /// when no compatibility state file exists yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
}

#[must_use]
fn command_next_step_with_references(
    argv: Vec<String>,
    description: impl Into<String>,
    references: Vec<String>,
) -> NextStep {
    NextStep {
        command: Some(render_command_for_display(&argv)),
        argv,
        description: description.into(),
        references,
    }
}

/// Build the default P5 agent-native handoff for every healthy sidecar state.
///
/// `workflow init` is intentionally the single bootstrap handoff: it is
/// idempotent and returns `AlreadyInitialized` when the ledger already exists.
/// The immediately following `workflow next` reference uses the same explicit
/// project root. Legacy operation-contract and preview material may remain as
/// secondary compatibility references, but never replaces this authority path.
#[must_use]
fn workflow_init_next_step(
    project_root: &Path,
    description: impl Into<String>,
    mut secondary_references: Vec<String>,
) -> NextStep {
    let root = project_root.display().to_string();
    let init_argv = vec![
        "forge-core".to_string(),
        "workflow".to_string(),
        "init".to_string(),
        "--root".to_string(),
        root.clone(),
    ];
    let next_argv = vec![
        "forge-core".to_string(),
        "workflow".to_string(),
        "next".to_string(),
        "--root".to_string(),
        root,
    ];
    secondary_references.insert(
        0,
        format!(
            "next: {} — derive the current governed action after idempotent initialization",
            render_command_for_display(&next_argv)
        ),
    );
    command_next_step_with_references(init_argv, description, secondary_references)
}

#[must_use]
fn render_command_for_display(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| quote_display_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
fn quote_display_arg(arg: &str) -> String {
    if arg.is_empty() || arg.chars().any(char::is_whitespace) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

impl From<ProjectResolvePayload> for ProjectContext {
    fn from(p: ProjectResolvePayload) -> Self {
        Self {
            project_id: p.project_id,
            project_root: p.project_root,
            link_path: p.link_path,
            sidecar_root: p.sidecar_root,
            state_root: p.state_root,
            state_exists: p.state_exists,
            layout: p.layout,
            current_phase: p.current_phase,
        }
    }
}

/// Hand-rolled error enum (project convention: no `anyhow`/`thiserror`).
/// `start` is read-only, so the failure modes are: the `--root` cannot be
/// resolved, or argv is malformed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// `--root` does not exist or is not a directory, or the project link is
    /// missing/malformed. Mirrors `resolve_project`'s failure surface.
    ProjectResolve {
        exit_reason: ExitReason,
        message: String,
    },
}

impl StartError {
    #[must_use]
    pub const fn exit_reason(&self) -> ExitReason {
        match self {
            Self::ProjectResolve { exit_reason, .. } => *exit_reason,
        }
    }
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectResolve { message, .. } => f.write_str(message),
        }
    }
}

impl std::error::Error for StartError {}

/// Runs the `forge-core start` subcommand.
///
/// Parses `--root` (default `.`), optional
/// `--agent-id`, and the standard `--json`/`--no-json` dual-output flags,
/// resolves the project, classifies the bootstrap state, and emits a
/// [`CliEnvelope<StartPayload>`] via
/// `emit`.
///
/// # Errors
///
/// Returns [`ExitError::usage`] on a malformed argv, and [`ExitError::with_code`]
/// (via `emit`) when project resolution fails and the envelope carries a
/// non-zero exit code.
pub fn run_start_command(args: &[String]) -> Result<(), ExitError> {
    let options = match parse_start_args(args)
        .map_err(|error| ExitError::usage(start_parse_error_with_usage(&error)))?
    {
        StartParseOutcome::Run(options) => options,
        StartParseOutcome::Help => {
            println!("{}", start_usage_line());
            return Ok(());
        }
    };

    let env = run_start_with_agent(&options.root, options.agent_id);
    crate::cli_util::emit_envelope(env, options.output.wants_json())
}

fn parse_start_args(args: &[String]) -> Result<StartParseOutcome, StartParseError> {
    let mut root = PathBuf::from(".");
    let mut agent_id: Option<String> = None;
    let mut output = StartOutputMode::Json;

    // argv[0] is the command name ("start"); subcommand args start at index 1,
    // matching the established parse loop in `eval_harness_cmd` / `project_cmd`.
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let value = require_start_value(args, index, "--root")?;
                root = PathBuf::from(value);
            }
            "--agent-id" | "--agent" => {
                index += 1;
                let value = require_start_value(args, index, "--agent-id")?;
                agent_id = Some(value);
            }
            "--json" => output = StartOutputMode::Json,
            "--no-json" | "--text" => output = StartOutputMode::Text,
            "--help" | "-h" => return Ok(StartParseOutcome::Help),
            other => {
                return Err(StartParseError::UnknownArgument {
                    argument: other.to_string(),
                });
            }
        }
        index += 1;
    }

    Ok(StartParseOutcome::Run(StartCliOptions {
        root,
        agent_id,
        output,
    }))
}

/// Pure core: resolve the project, classify the bootstrap state, build the
/// payload. Separated from the argv parser so it is unit-testable without
/// constructing argv strings.
///
/// `start` is read-only: this function performs no writes and acquires no
/// claim. Project resolution reuses [`resolve_project`] verbatim — `start`
/// does not duplicate the link/sidecar logic.
///
/// The one place `start` diverges from `resolve_project` is the
/// [`ProjectResolveError::MissingProjectLink`] case: instead of propagating a
/// bare env-config error, `start` returns an `ok` envelope in the `no_link`
/// bootstrap state with a concrete `next_step` of
/// `forge-core project init --root .`. That is the single most useful thing
/// `start` can tell a new user, and it was previously unreachable because
/// `resolve_project` failed before classification ran.
///
/// # Errors
///
/// Returns [`StartError::ProjectResolve`] when the project cannot be
/// resolved for any reason OTHER than a missing link (missing root,
/// malformed link, unsafe state root, …). The caller maps the [`ExitReason`]
/// into the envelope exit code.
#[must_use]
pub fn run_start(root: &Path) -> CliEnvelope<StartPayload> {
    run_start_with_agent(root, None)
}

/// Variant of [`run_start`] that preserves an optional host-supplied agent id
/// in the bootstrap payload. On a fresh repo it creates the Project Link and
/// sidecar. Once a link exists, missing or incomplete linked state fails closed
/// as possible durable-state loss and is never recreated automatically.
#[must_use]
pub fn run_start_with_agent(root: &Path, agent_id: Option<String>) -> CliEnvelope<StartPayload> {
    let observation = match resolve_project_observed(root) {
        Ok(observation) => observation,
        Err(ProjectResolveError::MissingProjectLink { .. }) => {
            // Fresh repo: no durable link claims prior initialization.
            return bootstrap_and_finish(root, agent_id);
        }
        Err(err) => {
            return CliEnvelope::err("start", resolve_error_exit_reason(&err), err.to_string());
        }
    };
    let project_link_sha256 = observation.project_link_sha256;
    let resolved = observation.payload;

    if let Some((cause, detail)) = linked_state_loss_detail(&resolved) {
        return state_loss_envelope(&resolved, agent_id, cause, &detail, &project_link_sha256);
    }

    finish_classified(&resolved, agent_id, Vec::new())
}

fn state_loss_envelope(
    resolved: &ProjectResolvePayload,
    agent_id: Option<String>,
    cause: StateLossCause,
    detail: &str,
    observed_project_link_sha256: &str,
) -> CliEnvelope<StartPayload> {
    // Never expose a digest for a substituted Project Link. For every other
    // cause, reuse the exact-byte digest from the same read that produced the
    // resolved identity rather than reopening a raceable path.
    let project_link_sha256 = (cause != StateLossCause::SymlinkSubstitution)
        .then(|| observed_project_link_sha256.to_string());
    let diagnosis_digest =
        state_loss_diagnosis_digest(resolved, cause, detail, project_link_sha256.as_deref());
    let reason = format!(
        "Project Link proves prior initialization, but {detail}; this is possible durable-state loss. Automatic recreation is forbidden. Inspect the link or select a future verified recovery path before proceeding."
    );
    let mut references = Vec::new();
    if let Some(link_path) = resolved.link_path.clone() {
        references.push(link_path);
    }
    references.push(resolved.state_root.clone());
    let project_root_display = resolved.project_root.clone();
    let payload = StartPayload {
        schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
        agent_id,
        state: BootstrapState::LinkPresentNoSidecar,
        reason: reason.clone(),
        next_step: Some(command_next_step_with_references(
            vec![
                "forge-core".to_string(),
                "project".to_string(),
                "resolve".to_string(),
                "--root".to_string(),
                project_root_display.clone(),
                "--json".to_string(),
            ],
            "Inspect Project Link resolution metadata without mutating project authority.",
            references,
        )),
        project: Some(ProjectContext::from(resolved.clone())),
        state_loss: Some(BootstrapStateLossDiagnostic {
            schema_version: BOOTSTRAP_STATE_LOSS_SCHEMA_VERSION.to_string(),
            diagnosis_digest,
            kind: StateLossKind::LinkedStateUnavailable,
            cause,
            project_id: resolved.project_id.clone(),
            project_link_schema_version: PROJECT_LINK_SCHEMA_VERSION.to_string(),
            project_link_sha256,
            workflow_release_id: None,
            workflow_release_status: StateLossReleaseStatus::UnavailableUntrustedState,
            choices: BootstrapRecoveryChoices::for_project_root(&project_root_display),
        }),
        actions_performed: Vec::new(),
    };
    CliEnvelope::reject("start", ExitReason::EnvConfig, reason, payload)
}

fn state_loss_diagnosis_digest(
    resolved: &ProjectResolvePayload,
    cause: StateLossCause,
    detail: &str,
    project_link_sha256: Option<&str>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"forge-bootstrap-state-loss-diagnosis-v1\0");
    for field in [
        resolved.project_id.as_str(),
        PROJECT_LINK_SCHEMA_VERSION,
        project_link_sha256.unwrap_or("unavailable"),
        cause.as_str(),
        detail,
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

/// Run first initialization, seed the authoritative compatibility state, then
/// re-resolve and classify the post-action state.
fn bootstrap_and_finish(root: &Path, agent_id: Option<String>) -> CliEnvelope<StartPayload> {
    match init_project(root, None, None, None) {
        Ok(init_payload) => {
            let initialized = init_payload.status == ProjectInitStatus::Initialized;
            let action = if initialized {
                "initialized"
            } else {
                "already_initialized"
            };
            // Seed compatibility state only for the initialization this call
            // just published. A Project Link that won a race is existing
            // authority and must remain read-only here.
            if initialized {
                if let Err(err) =
                    write_initial_project_state(std::path::Path::new(&init_payload.state_root))
                {
                    eprintln!("start: failed to seed state.yaml (non-fatal): {err}");
                }
            }
            match resolve_project(root) {
                Ok(resolved) => finish_classified(&resolved, agent_id, vec![action.to_string()]),
                Err(err) => {
                    CliEnvelope::err("start", resolve_error_exit_reason(&err), err.to_string())
                }
            }
        }
        Err(err) => CliEnvelope::err("start", init_error_exit_reason(&err), err.to_string()),
    }
}

/// Build the final ok envelope from a resolved, healthy project (sidecar
/// exists). No further bootstrap action; pure routing via `classify`.
fn finish_classified(
    resolved: &ProjectResolvePayload,
    agent_id: Option<String>,
    actions_performed: Vec<String>,
) -> CliEnvelope<StartPayload> {
    let project = ProjectContext::from(resolved.clone());
    let canonical_project_root = Path::new(&resolved.project_root);
    let (state, reason, next_step) = classify(resolved, canonical_project_root);
    CliEnvelope::ok(
        "start",
        StartPayload {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            agent_id,
            state,
            reason,
            next_step,
            project: Some(project),
            state_loss: None,
            actions_performed,
        },
    )
}

/// Classify a resolved healthy project. State-loss classification is handled
/// before this function so healthy routing cannot recreate or normalize it.
fn classify(
    resolved: &ProjectResolvePayload,
    project_root: &Path,
) -> (BootstrapState, String, Option<NextStep>) {
    // State tree healthy: look for compatibility evidence of progress on top
    // of it while keeping the same agent-native workflow handoff in every
    // healthy state. Preview and operation-contract signals affect only the
    // preserved BootstrapState wire value and secondary references.
    let has_preview = preview_has_run(resolved);
    let has_contract = operation_contract_present(project_root);

    if has_preview {
        return (
            BootstrapState::PreviewRun,
            "A preview has already been produced; the project is onboarded.".to_string(),
            Some(workflow_init_next_step(
                project_root,
                "Initialize or recover agent-native workflow governance; initialization is idempotent.",
                vec![
                    "compatibility: a preview trace already exists; preserve it as supporting material, not workflow authority."
                        .to_string(),
                    "start will keep reporting preview_run; the BootstrapState wire value remains stable."
                        .to_string(),
                ],
            )),
        );
    }

    if has_contract {
        return (
            BootstrapState::ContractPresent,
            "An operation contract exists; the project is ready for agent-native workflow governance."
                .to_string(),
            Some(workflow_init_next_step(
                project_root,
                "Initialize or recover agent-native workflow governance; initialization is idempotent.",
                vec![
                    "compatibility: forge-core preview --operation <your-contract> — validate the existing operation contract independently"
                        .to_string(),
                ],
            )),
        );
    }

    // State tree healthy, nothing on top of it yet. Agent-native workflow
    // governance is the default handoff. Starter operation fixtures remain as
    // secondary compatibility material and never select workflow or phase.
    (
        BootstrapState::SidecarReadyNoContract,
        "State tree is healthy but no operation contract exists yet.".to_string(),
        Some(workflow_init_next_step(
            project_root,
            "Initialize agent-native workflow governance; initialization is idempotent.",
            vec![
                format!(
                    "compatibility starter (observe): {OPERATION_CONTRACT_REFERENCE_DIR}{STARTER_FIXTURE_OBSERVE} — simplest read-only operation shape"
                ),
                format!(
                    "compatibility starter (execute): {OPERATION_CONTRACT_REFERENCE_DIR}{STARTER_FIXTURE_EXECUTE} — simplest write operation shape"
                ),
                format!("compatibility scenarios: {OPERATION_CONTRACT_REFERENCE_DIR}"),
                format!("compatibility validation: {OPERATION_VALIDATION_COMMAND}"),
            ],
        )),
    )
}

/// Heuristic: has a preview run? The runtime records M1 trace events under
/// the sidecar `traces/` dir (`create_state_tree` makes it). Non-empty
/// `traces/` is a conservative, read-only proxy for "preview has run". It
/// can under-report (a preview that wrote nothing), but never over-report —
/// which is the safe direction for a diagnostic.
fn preview_has_run(resolved: &ProjectResolvePayload) -> bool {
    let traces = PathBuf::from(&resolved.state_root).join("traces");
    dir_has_any_entry(&traces)
}

/// Heuristic: does an operation contract exist in the consumer repo? We look
/// for files whose name suggests an operation contract (contains "operation"
/// and ends in `.yaml`/`.yml`) at the repo root and one level under common
/// spec dirs. This is deliberately shallow and name-based: the authoritative
/// check is `preview` parsing the file, not `start` re-implementing that.
fn operation_contract_present(project_root: &Path) -> bool {
    let candidate_dirs = [
        project_root.to_path_buf(),
        project_root.join("contracts"),
        project_root.join("specs"),
        project_root.join("operations"),
    ];
    for dir in candidate_dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            let is_yaml = path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml")
            });
            if lower.contains("operation") && is_yaml {
                return true;
            }
        }
    }
    false
}

/// `read_dir` + "is there at least one entry" without collecting the whole
/// iterator. Returns `false` on any IO error (treated as "no evidence").
fn dir_has_any_entry(path: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };
    entries.flatten().next().is_some()
}

/// Map a [`ProjectResolveError`]'s exit reason onto the envelope taxonomy.
/// `start` does not invent new failure semantics — it propagates
/// `resolve_project`'s. Kept as a named adapter so F12.3/F12.4 enrichments
/// can attach state-specific guidance on resolution failures without
/// reaching into `project_cmd`'s private error variants.
fn resolve_error_exit_reason(err: &ProjectResolveError) -> ExitReason {
    err.exit_reason()
}

fn init_error_exit_reason(err: &ProjectInitError) -> ExitReason {
    err.exit_reason()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::PROJECT_LINK_FILE_NAME;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "forge-start-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_link(app: &Path, sidecar_rel: &str, state_rel: &str) {
        fs::write(
            app.join(forge_core_contracts::PROJECT_LINK_FILE_NAME),
            format!(
                "schema_version: forge_project_link_v1\n\
                 project_id: app\n\
                 sidecar_root: {sidecar_rel}\n\
                 state_root: {state_rel}\n",
            ),
        )
        .unwrap();
    }

    fn make_state_tree(state: &Path) {
        for d in [
            "",
            "artifacts",
            "claims-active",
            "evidence",
            "handoffs/expired-claims",
            "index",
            "locks",
            "traces",
            "wal",
        ] {
            fs::create_dir_all(state.join(d)).unwrap();
        }
        for f in [
            "ledger.ndjson",
            "wal/replay.fmr1",
            "replay-wal.manifest.json",
        ] {
            fs::write(state.join(f), b"").unwrap();
        }
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    fn expected_workflow_init_argv(root: &Path) -> Vec<String> {
        vec![
            "forge-core".to_string(),
            "workflow".to_string(),
            "init".to_string(),
            "--root".to_string(),
            root.display().to_string(),
        ]
    }

    fn assert_agent_native_healthy_next_step(next: &NextStep, root: &Path) {
        let expected = expected_workflow_init_argv(root);
        assert_eq!(next.argv, expected, "healthy start must emit exact argv");
        assert_eq!(
            next.command.as_deref(),
            Some(render_command_for_display(&expected).as_str()),
            "display command must be rendered from the structured argv"
        );
        let expected_next = render_command_for_display(&[
            "forge-core".to_string(),
            "workflow".to_string(),
            "next".to_string(),
            "--root".to_string(),
            root.display().to_string(),
        ]);
        assert!(
            next.references
                .first()
                .is_some_and(|reference| reference.contains(&expected_next)),
            "workflow next with the same explicit root must be the immediate reference: {:?}",
            next.references
        );
        assert!(
            next.description.to_ascii_lowercase().contains("idempotent"),
            "workflow init handoff must explain idempotence"
        );
        let projected = format!(
            "{} {} {}",
            next.command.as_deref().unwrap_or_default(),
            next.description,
            next.references.join(" ")
        )
        .to_ascii_lowercase();
        assert!(
            !projected.contains("guide describe") && !projected.contains("guide status"),
            "healthy next step must not recommend legacy guide routing: {projected}"
        );
    }

    fn assert_start_error_uses_command_surface_usage(message: &str, expected_diagnostic: &str) {
        assert!(
            message.contains(expected_diagnostic),
            "error should preserve diagnostic {expected_diagnostic:?}: {message}"
        );
        assert!(
            message.contains(start_usage_line()),
            "error should project start Command Surface usage {:?}: {message}",
            start_usage_line()
        );
        for unrelated_usage in [
            "forge-core project init",
            "forge-core project resolve",
            "forge-core mcp serve",
        ] {
            assert!(
                !message.contains(unrelated_usage),
                "start parse error should not leak unrelated usage {unrelated_usage:?}: {message}"
            );
        }
    }

    #[test]
    fn typed_start_parser_returns_options() {
        let parsed = parse_start_args(&argv(&[
            "start",
            "--root",
            "app",
            "--agent",
            "codex-main",
            "--no-json",
        ]))
        .expect("parse start args");

        let StartParseOutcome::Run(options) = parsed else {
            panic!("expected runnable start options");
        };
        assert_eq!(options.root, PathBuf::from("app"));
        assert_eq!(options.agent_id.as_deref(), Some("codex-main"));
        assert_eq!(options.output, StartOutputMode::Text);
    }

    #[test]
    fn typed_start_parser_short_circuits_help() {
        let parsed = parse_start_args(&argv(&["start", "--help"])).expect("parse help");
        assert_eq!(parsed, StartParseOutcome::Help);
    }

    #[test]
    fn typed_start_parser_reports_usage_errors() {
        let missing = parse_start_args(&argv(&["start", "--root"])).unwrap_err();
        assert_eq!(missing, StartParseError::MissingValue { flag: "--root" });
        assert_eq!(missing.to_string(), "start: --root requires a value");

        let flag_as_value =
            parse_start_args(&argv(&["start", "--root", "--agent-id"])).unwrap_err();
        assert_eq!(
            flag_as_value,
            StartParseError::FlagAsValue {
                flag: "--root",
                value: "--agent-id".to_string(),
            }
        );
        assert_eq!(
            flag_as_value.to_string(),
            "start: --root requires a value, got another flag '--agent-id'"
        );

        let unknown = parse_start_args(&argv(&["start", "--surprise"])).unwrap_err();
        assert_eq!(
            unknown,
            StartParseError::UnknownArgument {
                argument: "--surprise".to_string(),
            }
        );
        assert_eq!(
            unknown.to_string(),
            "start: unrecognized argument '--surprise'"
        );
    }

    #[test]
    fn start_parse_errors_use_start_usage() {
        let missing = parse_start_args(&argv(&["start", "--root"])).unwrap_err();
        let message = start_parse_error_with_usage(&missing);
        assert_start_error_uses_command_surface_usage(&message, "start: --root requires a value");

        let root_flag_as_value =
            parse_start_args(&argv(&["start", "--root", "--agent-id"])).unwrap_err();
        let message = start_parse_error_with_usage(&root_flag_as_value);
        assert_start_error_uses_command_surface_usage(
            &message,
            "start: --root requires a value, got another flag '--agent-id'",
        );

        let agent_flag_as_value =
            parse_start_args(&argv(&["start", "--agent-id", "--json"])).unwrap_err();
        assert_eq!(
            agent_flag_as_value,
            StartParseError::FlagAsValue {
                flag: "--agent-id",
                value: "--json".to_string(),
            }
        );
        let message = start_parse_error_with_usage(&agent_flag_as_value);
        assert_start_error_uses_command_surface_usage(
            &message,
            "start: --agent-id requires a value, got another flag '--json'",
        );

        let unknown = parse_start_args(&argv(&["start", "--surprise"])).unwrap_err();
        let message = start_parse_error_with_usage(&unknown);
        assert_start_error_uses_command_surface_usage(
            &message,
            "start: unrecognized argument '--surprise'",
        );
    }

    #[test]
    fn start_usage_line_projects_command_surface() {
        assert_eq!(
            start_usage_line(),
            COMMAND_START.canonical_usage().trim_start()
        );
    }

    #[test]
    fn bootstrap_state_wire_form_is_snake_case() {
        // Lock the wire contract: states serialize to snake_case to match the
        // rest of the CLI. Renaming a variant without updating consumers would
        // silently break the agent-facing payload.
        let cases = [
            (BootstrapState::NoLink, "no_link"),
            (
                BootstrapState::LinkPresentNoSidecar,
                "link_present_no_sidecar",
            ),
            (
                BootstrapState::SidecarReadyNoContract,
                "sidecar_ready_no_contract",
            ),
            (BootstrapState::ContractPresent, "contract_present"),
            (BootstrapState::PreviewRun, "preview_run"),
        ];
        for (state, expected) in cases {
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, format!("\"{expected}\""), "wire form for {state:?}");
            assert_eq!(state.as_str(), expected);
        }
    }

    #[test]
    fn no_link_bootstraps_the_project_in_one_command() {
        // State 1: repo with no Project Link and no bootstrap exception.
        // `start` now bootstraps the project (creates the link + sidecar) in
        // a single command, then reports the post-init state. This is the
        // single-command UX: the agent does not need a separate `project init`.
        let root = temp_root("no-link");
        let env = run_start(&root);
        assert!(env.ok, "no_link bootstraps and returns an ok envelope");
        assert_eq!(env.exit_reason.0, ExitReason::Ok.as_str());
        let payload = env.data.as_ref().expect("payload on no_link");
        // After bootstrap, the project is in sidecar_ready_no_contract.
        assert_eq!(
            payload.state,
            BootstrapState::SidecarReadyNoContract,
            "no_link should bootstrap and advance to sidecar_ready_no_contract"
        );
        assert_eq!(
            payload.actions_performed,
            vec!["initialized".to_string()],
            "no_link should report it initialized the project"
        );
        // The Project Link and sidecar were actually created.
        assert!(
            root.join(PROJECT_LINK_FILE_NAME).exists(),
            "start should create the Project Link on no_link"
        );
        assert!(
            payload.project.is_some(),
            "no_link has project context after bootstrap"
        );
        // The bootstrap compatibility phase is seeded at 1-discovery. P5
        // workflow authority begins in the separate ledger after workflow init.
        let project = payload.project.as_ref().expect("project context");
        assert_eq!(
            project.current_phase.as_deref(),
            Some("1-discovery"),
            "start should seed state.yaml with the 1-discovery entry phase"
        );
        assert_agent_native_healthy_next_step(
            payload.next_step.as_ref().expect("workflow handoff"),
            &root,
        );
    }

    #[test]
    fn no_link_bootstraps_project_with_space_in_path() {
        // Space-in-path must not break bootstrap. The link is created at the
        // raw path (no shell quoting); agents read `actions_performed` and
        // `state`, not a shell string.
        let root = temp_root("no link path");
        let env = run_start(&root);
        assert!(env.ok, "no_link with a space path should still exit zero");
        let payload = env.data.as_ref().expect("payload on no_link");
        assert_eq!(
            payload.state,
            BootstrapState::SidecarReadyNoContract,
            "no_link with space path should bootstrap and advance"
        );
        assert_eq!(
            payload.actions_performed,
            vec!["initialized".to_string()],
            "no_link with space path should report it initialized"
        );
        assert!(
            root.join(PROJECT_LINK_FILE_NAME).exists(),
            "start should create the Project Link even with a space in the path"
        );
        assert_agent_native_healthy_next_step(
            payload.next_step.as_ref().expect("workflow handoff"),
            &root,
        );
    }

    #[test]
    fn agent_id_is_preserved_in_bootstrap_payload() {
        let root = temp_root("agent-id");
        let env = run_start_with_agent(&root, Some("codex-main".to_string()));
        let payload = env.data.as_ref().expect("payload on no_link");
        assert_eq!(payload.agent_id.as_deref(), Some("codex-main"));
        // Bootstrap happens; state advances past no_link.
        assert_eq!(
            payload.state,
            BootstrapState::SidecarReadyNoContract,
            "agent_id test should observe post-bootstrap state"
        );
    }

    #[test]
    fn arbitrary_local_forge_method_without_core_markers_fails_closed() {
        // A repo with a local `.forge-method/` dir but no core markers and no
        // Project Link is in an unsafe state (consumer-local runtime state).
        // `start` does NOT silently create a Project Link on top of it —
        // `init_project` fails closed (`ConsumerLocalStateExists`), and `start`
        // surfaces that error so the operator cleans up before bootstrapping.
        let root = temp_root("local-state-not-core");
        make_state_tree(&root.join(".forge-method"));

        let env = run_start(&root);
        assert!(
            !env.ok,
            "unsafe local .forge-method should fail closed, not bootstrap"
        );
    }

    #[test]
    fn link_present_no_sidecar_fails_closed_without_recreating_state() {
        let parent = temp_root("no-sidecar-parent");
        let app = parent.join("app");
        let sidecar = parent.join("forge-app");
        fs::create_dir_all(&app).unwrap();
        write_link(&app, "../forge-app", "../forge-app/.forge-method");
        let link_path = app.join(PROJECT_LINK_FILE_NAME);
        let link_before = fs::read(&link_path).unwrap();

        let env = run_start(&app);
        let payload = env.data.as_ref().expect("typed state-loss payload");
        assert!(!env.ok, "linked missing state must fail closed");
        assert_eq!(env.exit_reason.0, ExitReason::EnvConfig.as_str());
        assert_eq!(payload.state, BootstrapState::LinkPresentNoSidecar);
        assert!(payload.actions_performed.is_empty());
        assert!(payload.reason.contains("possible durable-state loss"));
        assert!(payload.reason.contains("Automatic recreation is forbidden"));
        assert_eq!(fs::read(&link_path).unwrap(), link_before);
        assert!(
            !sidecar.exists(),
            "start must not recreate linked sidecar state"
        );
    }

    #[test]
    fn linked_empty_or_partial_state_fails_closed_without_repair() {
        for label in ["empty", "partial"] {
            let parent = temp_root(&format!("{label}-sidecar-parent"));
            let app = parent.join("app");
            let state = parent.join("forge-app").join(".forge-method");
            fs::create_dir_all(&app).unwrap();
            if label == "empty" {
                fs::create_dir_all(&state).unwrap();
            } else {
                make_state_tree(&state);
                fs::remove_dir(state.join("evidence")).unwrap();
            }
            write_link(&app, "../forge-app", "../forge-app/.forge-method");

            let env = run_start(&app);
            let payload = env.data.as_ref().expect("typed state-loss payload");
            assert!(!env.ok, "{label} linked state must fail closed");
            assert_eq!(payload.state, BootstrapState::LinkPresentNoSidecar);
            assert!(payload.actions_performed.is_empty());
            assert!(!state.join("evidence").exists());
        }
    }

    #[test]
    fn sidecar_ready_no_contract_diagnoses_state_three() {
        // State 3: healthy state tree, no operation contract, no preview.
        let parent = temp_root("ready-no-contract");
        let app = parent.join("app");
        let state = parent.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        make_state_tree(&state);
        write_link(&app, "../forge-app", "../forge-app/.forge-method");

        let env = run_start(&app);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::SidecarReadyNoContract);
        let next = payload.next_step.as_ref().expect("next step");
        assert_agent_native_healthy_next_step(next, &app);
        assert!(
            next.references
                .iter()
                .any(|r| r.contains(OPERATION_CONTRACT_REFERENCE_DIR)),
            "state 3 should point at reference scenarios"
        );
        assert!(
            next.references
                .iter()
                .any(|r| r.contains("preview --operation")),
            "state 3 should point at the validation command"
        );
        // F12.3 enrichment: state 3 must surface the two hand-picked starter
        // fixtures by name, so a new agent has an obvious entry point instead
        // of 23 undifferentiated files.
        assert!(
            next.references
                .iter()
                .any(|r| r.contains(STARTER_FIXTURE_OBSERVE)),
            "state 3 should name the observe starter fixture"
        );
        assert!(
            next.references
                .iter()
                .any(|r| r.contains(STARTER_FIXTURE_EXECUTE)),
            "state 3 should name the execute starter fixture"
        );
    }

    #[test]
    fn contract_present_diagnoses_state_four() {
        // State 4: state tree + an operation-contract-looking file.
        let parent = temp_root("with-contract");
        let app = parent.join("app");
        let state = parent.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        make_state_tree(&state);
        write_link(&app, "../forge-app", "../forge-app/.forge-method");
        // Drop a contract-looking file in the consumer repo.
        fs::write(app.join("my-operation.yaml"), "operation_contract: {}\n").unwrap();

        let env = run_start(&app);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::ContractPresent);
        let next = payload.next_step.as_ref().expect("next step");
        assert_agent_native_healthy_next_step(next, &app);
        // Existing operation material remains secondary compatibility evidence.
        assert!(
            next.references
                .iter()
                .any(|r| r.contains("preview --operation")),
            "state 4 should remind to validate the contract with preview"
        );
        assert!(
            !next.references.is_empty(),
            "state 4 references must not be empty (F12.4 enrichment)"
        );
    }

    #[test]
    fn preview_run_diagnoses_state_five() {
        // State 5: state tree + non-empty traces dir => preview has run.
        let parent = temp_root("preview-run");
        let app = parent.join("app");
        let state = parent.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        make_state_tree(&state);
        write_link(&app, "../forge-app", "../forge-app/.forge-method");
        // Simulate a trace having been written.
        fs::write(state.join("traces").join("m1.jsonl"), "{}\n").unwrap();

        let env = run_start(&app);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::PreviewRun);
        let next = payload.next_step.as_ref().expect("next step");
        assert_agent_native_healthy_next_step(next, &app);
        assert!(
            next.references
                .iter()
                .any(|r| r.contains("preview trace") && r.contains("not workflow authority")),
            "state 5 should retain preview evidence only as compatibility material"
        );
    }

    #[test]
    fn start_is_idempotent_running_twice_keeps_same_state() {
        // Running start twice must preserve both the BootstrapState and the
        // exact idempotent workflow-init handoff.
        let parent = temp_root("idempotent");
        let app = parent.join("app");
        let state = parent.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        make_state_tree(&state);
        write_link(&app, "../forge-app", "../forge-app/.forge-method");

        let first = run_start(&app);
        let second = run_start(&app);
        assert_eq!(
            first.data.as_ref().unwrap().state,
            second.data.as_ref().unwrap().state
        );
        assert_eq!(
            first.data.as_ref().unwrap().next_step,
            second.data.as_ref().unwrap().next_step
        );
        assert_agent_native_healthy_next_step(
            second
                .data
                .as_ref()
                .unwrap()
                .next_step
                .as_ref()
                .expect("workflow handoff"),
            &app,
        );
    }
}
