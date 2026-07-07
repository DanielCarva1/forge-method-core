//! `forge-core start` — F12 Guided Start: read-only bootstrap diagnostic.
//!
//! `start` advances a Consumer Project Repo from an empty state to the point
//! where the `guide` surface can take over. It is a *diagnostic*, not a
//! wizard: it inspects the real project state, classifies it into one of five
//! [`BootstrapState`]s, and emits a payload describing where the project is
//! and what the concrete next step is. It never executes effects and never
//! creates files — it composes with `project init` (which it *recommends*,
//! never invokes) and with `guide` (to which it *hands off* once
//! prerequisites exist).
//!
//! ## Why read-only
//!
//! Per the F12 grill (Option A on every question), the agent is the
//! bidirectional interpreter between human and product. The CLI emits a
//! payload; the agent decides the action. `start` carrying an effect side
//! (running `init`, writing a scaffold) would hide authority behind a
//! diagnostic and re-introduce the "adapter reinterprets Forge state" risk
//! the spec calls out. Read-only also means `start` needs no claim and no
//! `check-write`: it touches nothing governed.
//!
//! ## State machine
//!
//! The five states are documented as domain terms. Each maps to one concrete
//! next step:
//!
//! | state                         | next step                                  |
//! |-------------------------------|--------------------------------------------|
//! | `no_link`                     | `forge-core project init`                  |
//! | `link_present_no_sidecar`     | `forge-core project resolve` (diagnose)    |
//! | `sidecar_ready_no_contract`   | author a minimal operation contract        |
//! | `contract_present`            | `forge-core guide describe`                |
//! | `preview_run`                 | none (onboarded; use `guide`/`preview`)    |
//!
//! `start` recomputes from the real project on every call, so re-running
//! after an advance jumps to the correct state.
//!
//! ## Anti-script-de-novela (G1)
//!
//! `start` is parametric, not prescriptive: it adapts its output to the
//! project's real state and never dictates spec content. The
//! `sidecar_ready_no_contract` payload points at canonical reference
//! scenarios and the validation command, but the agent (with the human)
//! decides *what* to specify. The deletion test holds: state detection +
//! adaptive next-step selection is non-trivial behaviour, not a linear
//! script.

use std::path::{Path, PathBuf};

use forge_core_command_surface::COMMAND_START;
use forge_core_contracts::{CliEnvelope, ExitReason, ENVELOPE_SCHEMA_VERSION};

use crate::cli_error::ExitError;
use crate::project_cmd::{
    is_bootstrap_core_root, resolve_project, ProjectLayoutKind, ProjectResolveError,
    ProjectResolvePayload,
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
    allow_bootstrap_core: bool,
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

/// The `start` success payload. Carries the diagnosed state, the resolved
/// project context (when present), and the concrete next step for the agent
/// to act on or surface to the human.
///
/// Only `Serialize` is derived: this is an output payload emitted to stdout,
/// never deserialized back (mirrors `ProjectResolvePayload` / `StatusPayload`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StartPayload {
    /// Envelope contract version (matches the rest of the CLI).
    pub schema_version: String,
    /// Optional host-supplied agent identifier. `start` is read-only and does
    /// not acquire a claim; this field exists so host surfaces can correlate
    /// diagnostics without changing the bootstrap state machine.
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
}

/// The five bootstrap states a Consumer Project Repo can be in along the path
/// `start` diagnoses.
///
/// Wire form is `snake_case` to match the rest of the CLI contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapState {
    /// No Project Link present.
    NoLink,
    /// Link exists but the sidecar/state tree does not.
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
    pub bootstrap_core_exception: bool,
}

#[must_use]
fn command_next_step(argv: Vec<String>, description: impl Into<String>) -> NextStep {
    command_next_step_with_references(argv, description, Vec::new())
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
            bootstrap_core_exception: p.bootstrap_core_exception,
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
/// Parses `--root` (default `.`), `--allow-bootstrap-core`, optional
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

    let env = run_start_with_agent(
        &options.root,
        options.allow_bootstrap_core,
        options.agent_id,
    );
    crate::cli_util::emit_envelope(env, options.output.wants_json())
}

fn parse_start_args(args: &[String]) -> Result<StartParseOutcome, StartParseError> {
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
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
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
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
        allow_bootstrap_core,
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
pub fn run_start(root: &Path, allow_bootstrap_core: bool) -> CliEnvelope<StartPayload> {
    run_start_with_agent(root, allow_bootstrap_core, None)
}

/// Variant of [`run_start`] that preserves an optional host-supplied agent id
/// in the read-only diagnostic payload.
#[must_use]
pub fn run_start_with_agent(
    root: &Path,
    allow_bootstrap_core: bool,
    agent_id: Option<String>,
) -> CliEnvelope<StartPayload> {
    let resolved = match resolve_project(root, allow_bootstrap_core) {
        Ok(payload) => payload,
        Err(ProjectResolveError::MissingProjectLink { .. }) => {
            if is_bootstrap_core_root(root) {
                match resolve_project(root, true) {
                    Ok(payload) => {
                        return bootstrap_core_start_payload(root, payload, agent_id);
                    }
                    Err(err) => {
                        return CliEnvelope::err(
                            "start",
                            resolve_error_exit_reason(&err),
                            err.to_string(),
                        );
                    }
                }
            }
            // The repo has no `.forge-method.yaml`. This is the canonical
            // "brand new user" state — surface it as an actionable ok
            // envelope rather than an opaque env-config error.
            return CliEnvelope::ok(
                "start",
                StartPayload {
                    schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
                    agent_id,
                    state: BootstrapState::NoLink,
                    reason: "No Forge Project Link found; the repo is not yet \
                             governed by Forge."
                        .to_string(),
                    next_step: Some(command_next_step(
                        vec![
                            "forge-core".to_string(),
                            "project".to_string(),
                            "init".to_string(),
                            "--root".to_string(),
                            root.display().to_string(),
                        ],
                        "Create the Forge Project Link and the sibling \
                         Runtime Sidecar so Forge can govern writes.",
                    )),
                    project: None,
                },
            );
        }
        Err(err) => {
            return CliEnvelope::err("start", resolve_error_exit_reason(&err), err.to_string());
        }
    };

    let project = ProjectContext::from(resolved.clone());
    let (state, reason, next_step) = classify(&resolved, root);

    CliEnvelope::ok(
        "start",
        StartPayload {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            agent_id,
            state,
            reason,
            next_step,
            project: Some(project),
        },
    )
}

fn bootstrap_core_start_payload(
    root: &Path,
    resolved: ProjectResolvePayload,
    agent_id: Option<String>,
) -> CliEnvelope<StartPayload> {
    let project = ProjectContext::from(resolved.clone());
    let (state, reason, next_step) = classify(&resolved, root);
    let next_step = add_bootstrap_core_reference(next_step, root);
    CliEnvelope::ok(
        "start",
        StartPayload {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            agent_id,
            state,
            reason: format!(
                "Bootstrap Core Exception detected: no consumer Project Link is present, \
                 but this repository carries Forge core local state that resolves only \
                 with --allow-bootstrap-core. {reason}"
            ),
            next_step,
            project: Some(project),
        },
    )
}

fn add_bootstrap_core_reference(next_step: Option<NextStep>, root: &Path) -> Option<NextStep> {
    next_step.map(|mut step| {
        let command = render_command_for_display(&[
            "forge-core".to_string(),
            "project".to_string(),
            "resolve".to_string(),
            "--root".to_string(),
            root.display().to_string(),
            "--allow-bootstrap-core".to_string(),
            "--json".to_string(),
        ]);
        step.references
            .push(format!("verify bootstrap core resolution: {command}"));
        step
    })
}

/// Classify the bootstrap state from the resolved project. The ordering is
/// significant: we walk from the most-progressed signal downwards, so a
/// project that has both a state tree and a preview is reported as
/// `preview_run`, not `contract_present`.
///
/// `no_link` is handled by the caller (`resolve_project` fails before we get
/// here, so the only reachable states in this function assume a link
/// resolves). `link_present_no_sidecar` is reachable when the link parses
/// but `state_exists == false`.
fn classify(
    resolved: &ProjectResolvePayload,
    project_root: &Path,
) -> (BootstrapState, String, Option<NextStep>) {
    // State tree missing despite a (parseable) link: diagnose before progressing.
    if !resolved.state_exists {
        return (
            BootstrapState::LinkPresentNoSidecar,
            format!(
                "Project Link exists at {} but the state root {} does not; \
                 the sidecar tree is missing or has been removed.",
                resolved.link_path.as_deref().unwrap_or("<unresolved link>"),
                resolved.state_root
            ),
            Some(command_next_step_with_references(
                vec![
                    "forge-core".to_string(),
                    "project".to_string(),
                    "resolve".to_string(),
                    "--root".to_string(),
                    project_root.display().to_string(),
                ],
                "Diagnose the broken Project Link / sidecar before proceeding.",
                vec![resolved.state_root.clone()],
            )),
        );
    }

    // State tree healthy: look for evidence of progress on top of it. A
    // preview having run is the terminal "onboarded" signal; an operation
    // contract existing is the "ready for guide" signal.
    let has_preview = preview_has_run(resolved);
    let has_contract = operation_contract_present(project_root);

    if has_preview {
        return (
            BootstrapState::PreviewRun,
            "A preview has already been produced; the project is onboarded.".to_string(),
            Some(command_next_step_with_references(
                vec![
                    "forge-core".to_string(),
                    "guide".to_string(),
                    "describe".to_string(),
                ],
                "Project onboarded. start has nothing more to add; guide orients \
                 ongoing work (phases, workflows, gates).",
                vec![
                    "next: forge-core guide status --phase <current-phase>  \
                     — workflows eligible now + forward gate pending"
                        .to_string(),
                    "start will keep reporting preview_run; it is a terminal bootstrap state."
                        .to_string(),
                ],
            )),
        );
    }

    if has_contract {
        return (
            BootstrapState::ContractPresent,
            "An operation contract exists; the project is ready for the guide surface.".to_string(),
            Some(command_next_step_with_references(
                vec![
                    "forge-core".to_string(),
                    "guide".to_string(),
                    "describe".to_string(),
                ],
                "Hand off to guide; it orients phase, workflows, and gates. start's \
                 bootstrap job is done.",
                vec![
                    "then: forge-core guide status --phase discovery  \
                     — first phase; lists eligible workflows + the grill forward gate"
                        .to_string(),
                    "then: forge-core preview --operation <your-contract>  \
                     — validate the contract end-to-end before driving work"
                        .to_string(),
                ],
            )),
        );
    }

    // State tree healthy, nothing on top of it yet: the agent (with the
    // human) authors a minimal operation contract. start points at two
    // hand-picked starter fixtures (not the whole directory) and the
    // validation command, but does NOT generate the spec — the authority
    // boundary is the validated contract, not a template.
    (
        BootstrapState::SidecarReadyNoContract,
        "State tree is healthy but no operation contract exists yet.".to_string(),
        Some(NextStep {
            command: None,
            argv: Vec::new(),
            description: "Author a minimal operation contract modelled on a starter fixture, \
                          then validate it with preview."
                .to_string(),
            references: vec![
                format!(
                    "starter (observe): {OPERATION_CONTRACT_REFERENCE_DIR}{STARTER_FIXTURE_OBSERVE}  \
                     — simplest read-only shape"
                ),
                format!(
                    "starter (execute): {OPERATION_CONTRACT_REFERENCE_DIR}{STARTER_FIXTURE_EXECUTE}  \
                     — simplest write shape (shows authority.mutation_policy)"
                ),
                format!("more scenarios: {OPERATION_CONTRACT_REFERENCE_DIR}"),
                format!("validate with: {OPERATION_VALIDATION_COMMAND}"),
            ],
        }),
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

#[cfg(test)]
mod tests {
    use super::*;
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
            "traces",
            "wal",
        ] {
            fs::create_dir_all(state.join(d)).unwrap();
        }
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
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
            "--allow-bootstrap-core",
            "--agent",
            "codex-main",
            "--no-json",
        ]))
        .expect("parse start args");

        let StartParseOutcome::Run(options) = parsed else {
            panic!("expected runnable start options");
        };
        assert_eq!(options.root, PathBuf::from("app"));
        assert!(options.allow_bootstrap_core);
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
    fn no_link_returns_ok_with_project_init_next_step() {
        // State 1: repo with no Project Link and no bootstrap exception.
        // `start` must surface this as an actionable ok envelope (state
        // NoLink + next_step `forge-core project init --root .`), NOT as a
        // bare env-config error. This is the single most useful thing `start`
        // can tell a brand-new user.
        let root = temp_root("no-link");
        let env = run_start(&root, false);
        assert!(env.ok, "no_link must be an ok envelope, not an error");
        assert_eq!(env.exit_reason.0, ExitReason::Ok.as_str());
        let payload = env.data.as_ref().expect("payload on no_link");
        assert_eq!(payload.state, BootstrapState::NoLink);
        let next = payload.next_step.as_ref().expect("next step on no_link");
        assert!(
            next.command
                .as_ref()
                .is_some_and(|c| c.contains("project init")),
            "no_link should recommend `forge-core project init`; got {:?}",
            next.command
        );
        assert_eq!(
            next.argv,
            vec![
                "forge-core".to_string(),
                "project".to_string(),
                "init".to_string(),
                "--root".to_string(),
                root.display().to_string(),
            ],
            "no_link should expose typed argv so agents do not parse shell strings"
        );
        assert!(
            payload.project.is_none(),
            "no_link has no project context to report"
        );
    }

    #[test]
    fn no_link_next_step_quotes_space_paths_for_display() {
        let root = temp_root("no link path");
        let env = run_start(&root, false);
        let payload = env.data.as_ref().expect("payload on no_link");
        let next = payload.next_step.as_ref().expect("next step on no_link");
        let command = next.command.as_ref().expect("display command");
        let root_display = root.display().to_string();

        assert!(
            command.contains(&format!("--root \"{root_display}\"")),
            "display command should quote paths with spaces; got {command:?}"
        );
        assert_eq!(
            next.argv.last(),
            Some(&root_display),
            "typed argv should carry the raw path without shell quotes"
        );
    }

    #[test]
    fn agent_id_is_preserved_as_read_only_context() {
        let root = temp_root("agent-id");
        let env = run_start_with_agent(&root, false, Some("codex-main".to_string()));
        let payload = env.data.as_ref().expect("payload on no_link");
        assert_eq!(payload.agent_id.as_deref(), Some("codex-main"));
        assert_eq!(payload.state, BootstrapState::NoLink);
    }

    #[test]
    fn bootstrap_core_exception_is_diagnosed_without_consumer_link() {
        let root = temp_root("bootstrap-core");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/forge-core-cli\"]\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("crates").join("forge-core-cli")).unwrap();
        make_state_tree(&root.join(".forge-method"));

        let env = run_start(&root, false);
        let payload = env.data.as_ref().expect("payload on bootstrap core");
        let resolved = resolve_project(&root, true).expect("bootstrap core resolves explicitly");
        let expected_project = ProjectContext::from(resolved.clone());
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::SidecarReadyNoContract);
        assert!(
            payload.reason.contains("Bootstrap Core Exception"),
            "reason should name the explicit exception: {}",
            payload.reason
        );
        assert_eq!(
            payload.project.as_ref(),
            Some(&expected_project),
            "start must mirror `project resolve --allow-bootstrap-core` so agents do not infer a different project context"
        );
        assert_eq!(resolved.layout, ProjectLayoutKind::BootstrapCoreLocal);
        assert!(resolved.bootstrap_core_exception);
        let project = payload.project.as_ref().expect("project context");
        assert!(project.bootstrap_core_exception);
        assert_eq!(project.layout, ProjectLayoutKind::BootstrapCoreLocal);
        let next = payload.next_step.as_ref().expect("next step");
        assert!(
            next.references
                .iter()
                .any(|reference| reference.contains("project resolve")
                    && reference.contains("--allow-bootstrap-core")),
            "bootstrap-core next step should preserve the explicit resolve flag: {:?}",
            next.references
        );
    }

    #[test]
    fn arbitrary_local_forge_method_without_core_markers_remains_no_link() {
        let root = temp_root("local-state-not-core");
        make_state_tree(&root.join(".forge-method"));

        let env = run_start(&root, false);
        let payload = env.data.as_ref().expect("payload on no_link");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::NoLink);
        assert!(
            payload.project.is_none(),
            "local .forge-method alone must not be treated as a safe sidecar"
        );
    }

    #[test]
    fn link_present_no_sidecar_diagnoses_state_two() {
        // State 2: link parses but state root does not exist.
        let parent = temp_root("no-sidecar-parent");
        let app = parent.join("app");
        fs::create_dir_all(&app).unwrap();
        // sidecar dir intentionally NOT created.
        write_link(&app, "../forge-app", "../forge-app/.forge-method");

        let env = run_start(&app, false);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::LinkPresentNoSidecar);
        let next = payload.next_step.as_ref().expect("next step");
        assert!(
            next.command
                .as_ref()
                .is_some_and(|c| c.contains("project resolve")),
            "state 2 should recommend project resolve"
        );
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

        let env = run_start(&app, false);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::SidecarReadyNoContract);
        let next = payload.next_step.as_ref().expect("next step");
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
        assert!(
            next.command.is_none(),
            "state 3's step is authoring, not a command"
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

        let env = run_start(&app, false);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::ContractPresent);
        let next = payload.next_step.as_ref().expect("next step");
        assert_eq!(next.command.as_deref(), Some("forge-core guide describe"));
        // F12.4: state 4 references must point the agent past `guide describe`
        // to the concrete next actions (status discovery + preview validate).
        assert!(
            next.references
                .iter()
                .any(|r| r.contains("guide status --phase discovery")),
            "state 4 should point at guide status for the first phase"
        );
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

        let env = run_start(&app, false);
        let payload = env.data.as_ref().expect("payload on ok");
        assert!(env.ok);
        assert_eq!(payload.state, BootstrapState::PreviewRun);
        let next = payload.next_step.as_ref().expect("next step");
        // F12.4: state 5 is terminal bootstrap; it points at guide status
        // (ongoing orientation) and is explicit that start has nothing more.
        assert_eq!(next.command.as_deref(), Some("forge-core guide describe"));
        assert!(
            next.references
                .iter()
                .any(|r| r.contains("guide status --phase")),
            "state 5 should point at guide status for ongoing orientation"
        );
    }

    #[test]
    fn start_is_idempotent_running_twice_keeps_same_state() {
        // Read-only invariant: running start twice on the same repo must not
        // advance or regress the state. (If it wrote anything, the second
        // call could observe different state.)
        let parent = temp_root("idempotent");
        let app = parent.join("app");
        let state = parent.join("forge-app").join(".forge-method");
        fs::create_dir_all(&app).unwrap();
        make_state_tree(&state);
        write_link(&app, "../forge-app", "../forge-app/.forge-method");

        let first = run_start(&app, false);
        let second = run_start(&app, false);
        assert_eq!(
            first.data.as_ref().unwrap().state,
            second.data.as_ref().unwrap().state
        );
    }
}
