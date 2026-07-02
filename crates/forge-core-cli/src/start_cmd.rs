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
//! Per the F12 grill (Opção A on every question), the agent is the
//! bidirectional interpreter between human and product. The CLI emits a
//! payload; the agent decides the action. `start` carrying an effect side
//! (running `init`, writing a scaffold) would hide authority behind a
//! diagnostic and re-introduce the "adapter reinterprets Forge state" risk
//! the spec calls out. Read-only also means `start` needs no claim and no
//! `check-write`: it touches nothing governed.
//!
//! ## State machine
//!
//! The five states are documented as domain terms in `CONTEXT.md`
//! ("Start Bootstrap State"). Each maps to one concrete next step:
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

use forge_core_contracts::{CliEnvelope, ExitReason, ENVELOPE_SCHEMA_VERSION};

use crate::cli_error::ExitError;
use crate::project_cmd::{
    resolve_project, ProjectLayoutKind, ProjectResolveError, ProjectResolvePayload,
};

/// Usage line for `forge-core start`.
pub const START_USAGE_LINE: &str =
    "forge-core start [--root <path>] [--allow-bootstrap-core] [--json|--no-json]";

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
/// `start` diagnoses. Documented as a domain term in `CONTEXT.md`.
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
    /// A copy-pasteable / runnable command string, when one applies. Absent
    /// only when the step is not a single command (e.g. "author a spec",
    /// which is a creative act, not a command).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
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
/// Parses `--root` (default `.`), `--allow-bootstrap-core`, and the standard
/// `--json`/`--no-json` dual-output flags, resolves the project, classifies
/// the bootstrap state, and emits a [`CliEnvelope<StartPayload>`] via
/// [`emit`].
///
/// # Errors
///
/// Returns [`ExitError::usage`] on a malformed argv, and [`ExitError::with_code`]
/// (via [`emit`]) when project resolution fails and the envelope carries a
/// non-zero exit code.
pub fn run_start_command(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut allow_bootstrap_core = false;
    let mut want_json = true;

    // argv[0] is the command name ("start"); subcommand args start at index 1,
    // matching the established parse loop in `eval_harness_cmd` / `project_cmd`.
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage("start: --root requires a value"));
                };
                root = PathBuf::from(value);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--json" => want_json = true,
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("{START_USAGE_LINE}");
                return Ok(());
            }
            other => {
                return Err(ExitError::usage(format!(
                    "start: unrecognized argument '{other}'"
                )));
            }
        }
        index += 1;
    }

    let env = run_start(&root, allow_bootstrap_core);
    crate::cli_util::emit_envelope(env, want_json)
}

/// Pure core: resolve the project, classify the bootstrap state, build the
/// payload. Separated from the argv parser so it is unit-testable without
/// constructing argv strings.
///
/// `start` is read-only: this function performs no writes and acquires no
/// claim. Project resolution reuses [`resolve_project`] verbatim — `start`
/// does not duplicate the link/sidecar logic.
///
/// # Errors
///
/// Returns [`StartError::ProjectResolve`] when the project cannot be
/// resolved (missing root, missing/malformed link, unsafe state root, …).
/// The caller maps the [`ExitReason`] into the envelope exit code.
#[must_use]
pub fn run_start(root: &Path, allow_bootstrap_core: bool) -> CliEnvelope<StartPayload> {
    let resolved = match resolve_project(root, allow_bootstrap_core) {
        Ok(payload) => payload,
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
            state,
            reason,
            next_step,
            project: Some(project),
        },
    )
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
            Some(NextStep {
                command: Some(format!(
                    "forge-core project resolve --root {}",
                    project_root.display()
                )),
                description: "Diagnose the broken Project Link / sidecar before proceeding."
                    .to_string(),
                references: vec![resolved.state_root.clone()],
            }),
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
            Some(NextStep {
                command: Some("forge-core guide describe".to_string()),
                description: "Project onboarded. start has nothing more to add; guide orients \
                              ongoing work (phases, workflows, gates)."
                    .to_string(),
                references: vec![
                    "next: forge-core guide status --phase <current-phase>  \
                     — workflows eligible now + forward gate pending"
                        .to_string(),
                    "start will keep reporting preview_run; it is a terminal bootstrap state."
                        .to_string(),
                ],
            }),
        );
    }

    if has_contract {
        return (
            BootstrapState::ContractPresent,
            "An operation contract exists; the project is ready for the guide surface.".to_string(),
            Some(NextStep {
                command: Some("forge-core guide describe".to_string()),
                description: "Hand off to guide; it orients phase, workflows, and gates. start's \
                              bootstrap job is done."
                    .to_string(),
                references: vec![
                    "then: forge-core guide status --phase discovery  \
                     — first phase; lists eligible workflows + the grill forward gate"
                        .to_string(),
                    "then: forge-core preview --operation <your-contract>  \
                     — validate the contract end-to-end before driving work"
                        .to_string(),
                ],
            }),
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
            .map(|d| d.as_nanos())
            .unwrap_or(0);
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

    #[test]
    fn bootstrap_state_wire_form_is_snake_case() {
        // Lock the wire contract: states serialize to snake_case to match the
        // rest of the CLI. Renaming a variant without updating CONTEXT.md /
        // consumers would silently break the agent-facing payload.
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
    fn no_link_returns_env_config_error() {
        // State 1: repo with no Project Link and no bootstrap exception.
        let root = temp_root("no-link");
        let env = run_start(&root, false);
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, ExitReason::EnvConfig.as_str());
        assert_eq!(env.exit_code(), ExitReason::EnvConfig.as_code());
        assert!(
            env.data.is_none(),
            "no_link must not build a payload (no project context to report)"
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
