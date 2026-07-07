//! `guide` command family — the agent-first method surface (slice 3).
//!
//! These commands are the PRIMARY consumer of host LLMs. Every command emits a
//! single [`CliEnvelope`] as JSON to stdout; diagnostics go to stderr.
//! Implements R1/R3/R4 from the slice-3 spec.

use forge_core_command_surface::COMMAND_GUIDE;
use crate::project_cmd::resolve_project;
use forge_core_contracts::{
    Catalog, CatalogEntry, CliEnvelope, ExitReason, Phase, ENVELOPE_SCHEMA_VERSION,
};
use forge_core_contracts::{GuideDecision, GuideDecisionDocument};
use forge_core_decisions::{load_catalog, load_embedded_catalog, CatalogLoadReport};
use forge_core_decisions::{
    validate_guide_decision, GateKind, GuideRejection, GuideValidation, ProvidedGateResult,
};
use std::path::Path;

use crate::cli_error::ExitError;

// ============================================================================
// guide describe — the compact routing surface (R3 token cliff, DD13).
// ============================================================================

/// One compact workflow row in `describe`. Deliberately small: id, phase tags,
/// one-line description derived from the first trigger. The host reads this
/// ONCE per session and never re-reads unless `schema_version` changes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DescribeWorkflow {
    pub id: String,
    pub phases: Vec<String>,
    pub summary: String,
}

/// One gate row in `describe` — the phase transitions that require it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DescribeGate {
    pub gate: String,
    pub required_for: Vec<String>,
}

/// The full `describe` payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DescribePayload {
    pub schema_version: String,
    pub phases: Vec<String>,
    pub workflows: Vec<DescribeWorkflow>,
    pub gates: Vec<DescribeGate>,
    pub exit_reasons: Vec<String>,
}

impl DescribePayload {
    /// Build the describe payload from a loaded catalog + the static gate map.
    #[must_use]
    pub fn from_catalog(catalog: &Catalog) -> Self {
        let workflows = catalog
            .entries
            .iter()
            .map(compact_workflow)
            .collect::<Vec<_>>();
        Self {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            phases: Phase::ALL.iter().map(Phase::to_string).collect(),
            workflows,
            gates: gate_table(),
            exit_reasons: vec![
                "ok".into(),
                "rejected_by_gate".into(),
                "invalid_decision_shape".into(),
                "conflict".into(),
                "env_config".into(),
            ],
        }
    }
}

/// Compress a catalog entry to the compact describe row.
fn compact_workflow(e: &CatalogEntry) -> DescribeWorkflow {
    // summary = first trigger (the matching predicate) is the most concise
    // intent signal available without loading the full workflow text.
    let summary = e
        .triggers
        .first()
        .cloned()
        .unwrap_or_else(|| format!("workflow {}", e.id.0));
    DescribeWorkflow {
        id: e.id.0.clone(),
        phases: e.phases.iter().map(|p| p.0.clone()).collect(),
        summary,
    }
}

/// The static map of which gate is required for which forward transition.
/// Kept in lockstep with forge-core-decisions::phase_transition::required_gate_for.
fn gate_table() -> Vec<DescribeGate> {
    vec![
        DescribeGate {
            gate: "grill-gate".into(),
            required_for: vec!["1-discovery -> 2-specification".into()],
        },
        DescribeGate {
            gate: "system-design".into(),
            required_for: vec!["2-specification -> 3-plan".into()],
        },
    ]
}

/// Run `guide describe`. Loads the catalog from `catalog_dir` and emits the
/// compact routing surface.
///
/// # Errors
/// Returns an error envelope (exit 5) if the catalog directory cannot be read
/// or any workflow file is malformed.
#[must_use]
/// Resolve the catalog source:
/// - `Some(dir)` → load that directory from disk (explicit `--catalog-dir`).
/// - `None` → fall through: if a local `contracts/workflows/` exists in the
///   current working directory, use it (brownfield/forge workspace); otherwise
///   load the catalog embedded in the binary (greenfield, zero-config).
///
/// This is the fix for the greenfield blocker: a freshly installed
/// `forge-core` binary now carries its 110 workflows inside it, so
/// `guide status` works on any machine without `--catalog-dir`.
fn resolve_catalog(catalog_dir: Option<&Path>) -> CatalogLoadReport {
    if let Some(dir) = catalog_dir {
        load_catalog(dir)
    } else {
        let local = Path::new("contracts/workflows");
        if local.is_dir() {
            load_catalog(local)
        } else {
            load_embedded_catalog()
        }
    }
}

#[must_use]
pub fn run_describe(catalog_dir: Option<&Path>) -> CliEnvelope<DescribePayload> {
    let report = resolve_catalog(catalog_dir);
    if !report.is_clean() {
        return CliEnvelope::err(
            "guide.describe",
            ExitReason::EnvConfig,
            format!("catalog load failed: {} error(s)", report.errors.len()),
        );
    }
    let payload = DescribePayload::from_catalog(&report.catalog);
    CliEnvelope::ok("guide.describe", payload)
}

// Re-export the load report type for callers that want the raw errors.
pub type DescribeReport = CatalogLoadReport;

// ============================================================================
// guide decide — validate a host-proposed GuideDecision (R2).
// ============================================================================

/// The success payload for `guide decide` when the decision is Accepted.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DecideAccepted {
    pub recommended_workflow: String,
    pub current_phase: String,
    pub proposed_next_phase: Option<String>,
    pub reason: String,
    /// V5 — binding enforcement policy the agent must satisfy to advance this
    /// workflow. Populated when `--root` resolves a project; `None` when no
    /// project context is available (the legacy behavior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforcement_policy: Option<EnforcementPolicy>,
}

/// The binding policy a `guide decide` acceptance carries: what the agent
/// must do (acquire a claim, satisfy gates) before the runtime will execute
/// the recommended workflow. This is the "orchestrator" output — the agent
/// reads it and complies, rather than guessing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnforcementPolicy {
    /// `true` when the recommended workflow mutates durable state and the
    /// runtime's ClaimCoverageGate will refuse execution without a covering
    /// claim. The agent should `claim acquire` before calling execute-operation.
    pub claim_required: bool,
    /// Autonomy lane derived from the project's phase (Fast for discovery/spec,
    /// Rigorous for build-verify and beyond). Advisory — the runtime enforces
    /// via ClaimCoverageGate + PhaseGate, not via this string.
    pub lane: String,
    /// Gates the runtime will attach automatically (the agent does not pass
    /// these as flags). Informative so the agent knows what will be checked.
    pub automatic_gates: Vec<String>,
}

/// The failure payload for `guide decide` when the decision is Rejected.
/// Carries a machine-readable reject code so the host can self-correct (R2).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DecideRejected {
    /// One of: `unrecognized_current_phase` | `unknown_workflow` |
    /// `not_eligible_in_phase` | `illegal_transition`.
    pub reject_code: String,
    pub detail: String,
}

/// Run `guide decide`. Loads decision + catalog + gates, validates, emits
/// Accepted|Rejected envelope with the DD10 exit code.
///
/// # Errors
/// Returns an `InvalidDecisionShape` (exit 3) envelope if the decision file
/// cannot be deserialized; `EnvConfig` (exit 5) if the catalog won't load;
/// `RejectedByGate` (exit 2) if the engine refuses the decision.
#[must_use]
pub fn run_decide(
    decision_file: &Path,
    catalog_dir: Option<&Path>,
    gates: &[ProvidedGateResult],
    project: Option<&crate::project_cmd::ProjectResolvePayload>,
) -> CliEnvelope<DecideAccepted> {
    // 1. Load the decision (typed). Shape error -> exit 3.
    let decision_text = match std::fs::read_to_string(decision_file) {
        Ok(t) => t,
        Err(e) => {
            return CliEnvelope::err(
                "guide.decide",
                ExitReason::InvalidDecisionShape,
                format!("cannot read decision file: {e}"),
            );
        }
    };
    let decision: GuideDecision =
        match yaml_serde::from_str::<GuideDecisionDocument>(&decision_text) {
            Ok(doc) => doc.guide_decision,
            Err(e) => {
                return CliEnvelope::err(
                    "guide.decide",
                    ExitReason::InvalidDecisionShape,
                    format!("decision file is not a valid GuideDecisionDocument: {e}"),
                );
            }
        };

    // 2. Load the catalog.
    let report = resolve_catalog(catalog_dir);
    if !report.is_clean() {
        return CliEnvelope::err(
            "guide.decide",
            ExitReason::EnvConfig,
            format!("catalog load failed: {} error(s)", report.errors.len()),
        );
    }

    // 3. Validate against catalog + gates.
    match validate_guide_decision(&decision, &report.catalog, gates) {
        GuideValidation::Accepted => CliEnvelope::ok(
            "guide.decide",
            DecideAccepted {
                recommended_workflow: decision.recommended_workflow.0.clone(),
                current_phase: decision.current_phase.0.clone(),
                proposed_next_phase: decision.proposed_next_phase.as_ref().map(|p| p.0.clone()),
                reason: decision.reason.clone(),
                enforcement_policy: project.map(resolve_enforcement_policy),
            },
        ),
        GuideValidation::Rejected(reason) => {
            // The reject envelope carries a typed code + detail (R2: host self-corrects).
            // Encode in error field; exit reason = RejectedByGate (2).
            let rejected = DecideRejected {
                reject_code: reject_code(&reason),
                detail: format!("{reason:?}"),
            };
            let mut env: CliEnvelope<DecideAccepted> =
                CliEnvelope::err("guide.decide", ExitReason::RejectedByGate, &rejected.detail);
            // stash the typed reject payload in the error code for machine parsing.
            if let Some(err) = env.error.as_mut() {
                err.code.0 = format!("{}:{}", rejected.reject_code, rejected.detail);
            }
            env
        }
    }
}

/// Map a resolved project to a binding enforcement policy. The policy tells
/// the agent what the runtime will require before executing the recommended
/// workflow: a covering claim (for durable mutation in build-verify and
/// beyond) and the autonomy lane (Fast/Rigorous, derived from phase). The
/// agent reads this and complies; the runtime enforces via ClaimCoverageGate
/// and PhaseGate regardless.
fn resolve_enforcement_policy(project: &crate::project_cmd::ProjectResolvePayload) -> EnforcementPolicy {
    let phase = project
        .current_phase
        .as_deref()
        .map_or(Phase::Discovery, |raw| {
            Phase::parse(raw).unwrap_or(Phase::Discovery)
        });
    // Durable mutation is gated from build-verify onward; discovery/spec/plan
    // are human-heavy and the agent moves fast there (no claim required).
    let claim_required = matches!(
        phase,
        Phase::BuildVerify | Phase::ReadyOperate | Phase::Evolve
    );
    let lane = if matches!(phase, Phase::BuildVerify | Phase::ReadyOperate | Phase::Evolve) {
        "rigorous"
    } else {
        "fast"
    };
    // The runtime attaches these automatically based on project state (FASE 2).
    let automatic_gates = vec!["claim-coverage".to_string(), "phase".to_string()];
    EnforcementPolicy {
        claim_required,
        lane: lane.to_string(),
        automatic_gates,
    }
}

/// Map a [`GuideRejection`] to a stable machine-readable code string.
fn reject_code(r: &GuideRejection) -> String {
    match r {
        GuideRejection::UnrecognizedCurrentPhase { .. } => "unrecognized_current_phase",
        GuideRejection::UnknownWorkflow { .. } => "unknown_workflow",
        GuideRejection::NotEligibleInPhase { .. } => "not_eligible_in_phase",
        GuideRejection::IllegalTransition(_) => "illegal_transition",
        GuideRejection::UnrecognizedProposedPhase { .. } => "unrecognized_proposed_phase",
    }
    .into()
}

// ============================================================================
// guide status — orient the host: phase + eligible workflows + pending gates.
// ============================================================================

/// The `guide status` payload. Tells the host WHERE it is and WHAT it may do next.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StatusPayload {
    pub schema_version: String,
    /// The phase this status is oriented to.
    pub current_phase: String,
    /// Workflows eligible in `current_phase` (id + phases).
    pub eligible_workflows: Vec<StatusWorkflow>,
    /// Gates required to move FORWARD out of this phase, if any.
    pub pending_gates: Vec<StatusGate>,
    /// The phase each pending gate unlocks.
    pub next_phases: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StatusWorkflow {
    pub id: String,
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StatusGate {
    pub gate: String,
    pub unlocks: String,
}

/// Run `guide status` for a given phase. The host passes its current phase
/// (it always knows it from the method protocol); the engine reports what is
/// eligible now and which gates gate forward progress.
///
/// # Errors
/// Returns `EnvConfig` (exit 5) if the catalog won't load;
/// `InvalidDecisionShape` (exit 3) if `phase` does not categorize.
#[must_use]
pub fn run_status(catalog_dir: Option<&Path>, phase: &str) -> CliEnvelope<StatusPayload> {
    // categorize phase
    let Some(current) = Phase::parse(phase) else {
        return CliEnvelope::err(
            "guide.status",
            ExitReason::InvalidDecisionShape,
            format!("unrecognized phase '{phase}'"),
        );
    };

    let report = resolve_catalog(catalog_dir);
    if !report.is_clean() {
        return CliEnvelope::err(
            "guide.status",
            ExitReason::EnvConfig,
            format!("catalog load failed: {} error(s)", report.errors.len()),
        );
    }

    let eligible_workflows = report
        .catalog
        .entries
        .iter()
        .filter(|e| {
            e.phases
                .iter()
                .any(|tag| Phase::tag_eligible(&tag.0, current))
        })
        .map(|e| StatusWorkflow {
            id: e.id.0.clone(),
            phases: e.phases.iter().map(|p| p.0.clone()).collect(),
        })
        .collect::<Vec<_>>();

    let (pending_gates, next_phases) = forward_gates_for(current);

    CliEnvelope::ok(
        "guide.status",
        StatusPayload {
            schema_version: ENVELOPE_SCHEMA_VERSION.to_string(),
            current_phase: current.to_string(),
            eligible_workflows,
            pending_gates,
            next_phases,
        },
    )
}

/// The forward gate + destination for a phase, in lockstep with `phase_transition`.
fn forward_gates_for(phase: Phase) -> (Vec<StatusGate>, Vec<String>) {
    use forge_core_decisions::GateKind;
    let (gate, unlocks) = match phase {
        Phase::Discovery => (Some(GateKind::Grill), Some(Phase::Specification)),
        Phase::Specification => (Some(GateKind::SystemDesign), Some(Phase::Plan)),
        Phase::Plan => (Some(GateKind::StoryReady), Some(Phase::BuildVerify)),
        Phase::BuildVerify => (Some(GateKind::Readiness), Some(Phase::ReadyOperate)),
        _ => (None, None),
    };
    let pending_gates = gate
        .map(|g| StatusGate {
            gate: gate_str(g),
            unlocks: unlocks.unwrap().to_string(),
        })
        .into_iter()
        .collect();
    let next_phases = unlocks.map(|p| vec![p.to_string()]).unwrap_or_default();
    (pending_gates, next_phases)
}

fn gate_str(g: GateKind) -> String {
    match g {
        GateKind::Grill => "grill".into(),
        GateKind::SystemDesign => "system-design".into(),
        GateKind::StoryReady => "story-ready".into(),
        GateKind::Readiness => "readiness".into(),
        GateKind::Release => "release".into(),
    }
}
/// Dispatch entrypoint for the `forge-core guide` subcommand tree.
///
/// Routes to `describe`, `decide`, or `status` based on `args[1]`, and
/// prints usage on `--help` / unknown subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` when the subcommand is unknown. Sub-command
/// dispatchers may surface their own `ExitError::usage`,
/// `ExitError::invalid_value`, or `ExitError::with_code` variants.
pub fn run_guide_command(args: &[String]) -> Result<(), ExitError> {
    // Subcommand: `forge-core guide <subcommand> [...]`.
    let sub = args.get(1).map_or("--help", String::as_str);

    match sub {
        "describe" => run_guide_describe(&args[2..]),
        "decide" => run_guide_decide(&args[2..]),
        "status" => run_guide_status(&args[2..]),
        "--help" | "-h" | "help" => {
            print_guide_usage();
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core guide: unknown subcommand '{other}'. Try: {hint}",
            hint = guide_subcommand_hint()
        ))),
    }
}

fn print_guide_usage() {
    println!("forge-core guide <subcommand> [options]");
    for line in COMMAND_GUIDE.local_usage_lines() {
        println!("  {line}");
    }
}

fn guide_subcommand_hint() -> String {
    COMMAND_GUIDE.concrete_subcommand_hint()
}

fn guide_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_GUIDE
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core guide <subcommand> [options]")
}

fn guide_invalid_value_with_usage(subcommand: &str, message: &str) -> ExitError {
    ExitError::invalid_value(format!(
        "{message}\n\nusage:\n  {}",
        guide_command_surface_usage_line_for(subcommand)
    ))
}

pub fn guide_value(args: &[String], idx: usize) -> Option<&str> {
    args.get(idx)
        .filter(|value| !value.is_empty() && !value.starts_with("--"))
        .map(String::as_str)
}

/// Reads the value at `args[idx]`, returning `None` when missing, empty,
/// or starting with `--` (i.e. looks like the next flag).
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when [`guide_value`] returns `None`,
/// i.e. the slot at `idx` is missing, empty, or starts with `--`.
pub fn require_guide_value(
    args: &[String],
    idx: usize,
    subcommand: &str,
    flag: &str,
) -> Result<String, ExitError> {
    if let Some(value) = guide_value(args, idx) {
        Ok(value.to_owned())
    } else {
        let message = format!("guide {subcommand}: --{flag} requires a value");
        Err(guide_invalid_value_with_usage(subcommand, &message))
    }
}

#[must_use]
pub fn reject_unknown_guide_arg(subcommand: &str, arg: &str) -> ExitError {
    let message = format!("guide {subcommand}: unrecognized argument '{arg}'");
    eprintln!("{message}");
    guide_invalid_value_with_usage(subcommand, &message)
}

/// Runs the `forge-core guide describe` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when an argument is missing or
/// unrecognized, and `ExitError::with_code` (via [`emit_guide`]) when the
/// describe envelope carries a non-zero exit code.
pub fn run_guide_describe(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::CliEnvelope;

    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "describe",
                    "catalog-dir",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("describe"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("describe", other)),
        }
        idx += 1;
    }

    let env: CliEnvelope<DescribePayload> = run_describe(catalog_dir.as_deref());
    emit_guide(env, want_json)
}

/// Runs the `forge-core guide decide` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `--decision-file` is missing
/// or an argument is unrecognized, and `ExitError::with_code` (via
/// [`emit_guide`]) when the decide envelope carries a non-zero exit code.
pub fn run_guide_decide(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::CliEnvelope;

    let mut decision_file: Option<std::path::PathBuf> = None;
    let mut root: Option<std::path::PathBuf> = None;
    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut gates_file: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--decision-file" => {
                idx += 1;
                decision_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "decision-file",
                )?));
            }
            "--root" => {
                idx += 1;
                root = Some(std::path::PathBuf::from(require_guide_value(
                    args, idx, "decide", "root",
                )?));
            }
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "catalog-dir",
                )?));
            }
            "--gates-file" => {
                idx += 1;
                gates_file = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "decide",
                    "gates-file",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("decide"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("decide", other)),
        }
        idx += 1;
    }

    let decision_file = decision_file.ok_or_else(|| {
        let message = "guide decide: --decision-file is required";
        eprintln!("{message}");
        guide_invalid_value_with_usage("decide", message)
    })?;

    // Gates are optional (only needed for phase transitions). Loaded from a simple
    // YAML file: [{gate_kind: system-design, status: pass}, ...].
    let gates = load_gates(gates_file.as_deref());

    // Resolve project state (for the enforcement policy). When --root is
    // absent or the project isn't bootstrapped, the policy stays None and the
    // legacy (advisory) decide behavior is preserved.
    let project = root
        .as_deref()
        .and_then(|r| resolve_project(r).ok());

    let env: CliEnvelope<DecideAccepted> =
        run_decide(&decision_file, catalog_dir.as_deref(), &gates, project.as_ref());
    emit_guide(env, want_json)
}

/// Runs the `forge-core guide status` subcommand.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `--phase` is missing or an
/// argument is unrecognized, and `ExitError::with_code` (via [`emit_guide`])
/// when the status envelope carries a non-zero exit code.
pub fn run_guide_status(args: &[String]) -> Result<(), ExitError> {
    use forge_core_contracts::CliEnvelope;

    let mut phase: Option<String> = None;
    let mut root: Option<std::path::PathBuf> = None;
    let mut catalog_dir: Option<std::path::PathBuf> = None;
    let mut want_json = true;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--phase" => {
                idx += 1;
                phase = Some(require_guide_value(args, idx, "status", "phase")?);
            }
            "--root" => {
                idx += 1;
                root = Some(std::path::PathBuf::from(require_guide_value(
                    args, idx, "status", "root",
                )?));
            }
            "--catalog-dir" => {
                idx += 1;
                catalog_dir = Some(std::path::PathBuf::from(require_guide_value(
                    args,
                    idx,
                    "status",
                    "catalog-dir",
                )?));
            }
            "--no-json" | "--text" => want_json = false,
            "--json" => want_json = true,
            "--help" | "-h" => {
                println!("{}", guide_command_surface_usage_line_for("status"));
                return Ok(());
            }
            other => return Err(reject_unknown_guide_arg("status", other)),
        }
        idx += 1;
    }

    // Phase authority: an explicit `--phase` wins (host override). Otherwise
    // read the authoritative phase from the project's `state.yaml`. If no root
    // is given or the state file is missing, fall back to `1-discovery` so a
    // freshly-bootstrapped project still has a usable funnel entry point.
    let phase = phase.unwrap_or_else(|| resolve_current_phase(root.as_deref()));

    let env: CliEnvelope<StatusPayload> = run_status(catalog_dir.as_deref(), &phase);
    emit_guide(env, want_json)
}

/// Resolve the authoritative current phase for a project root. Reads
/// `<state_root>/state.yaml` via `resolve_project`; on any failure or missing
/// file, returns `"1-discovery"` as the funnel entry point. This makes the
/// runtime the phase authority rather than trusting whatever the agent passes
/// on `--phase`.
fn resolve_current_phase(root: Option<&std::path::Path>) -> String {
    const DEFAULT_PHASE: &str = "1-discovery";
    let Some(root) = root else {
        return DEFAULT_PHASE.to_string();
    };
    match resolve_project(root) {
        Ok(payload) => payload
            .current_phase
            .unwrap_or_else(|| DEFAULT_PHASE.to_string()),
        Err(_) => DEFAULT_PHASE.to_string(),
    }
}

/// Parse the gates-file into `ProvidedGateResult` rows. Empty/absent = no gates provided.
#[must_use]
pub fn load_gates(path: Option<&std::path::Path>) -> Vec<forge_core_decisions::ProvidedGateResult> {
    use forge_core_contracts::gate::GateStatus;
    use forge_core_decisions::GateKind;
    let Some(path) = path else {
        return Vec::new();
    };
    #[allow(clippy::items_after_statements)]
    #[derive(serde::Deserialize)]
    struct GateRow {
        gate_kind: String,
        status: String,
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let rows: Vec<GateRow> = yaml_serde::from_str(&text).unwrap_or_default();
    rows.into_iter()
        .filter_map(|r| {
            let gk = match r.gate_kind.as_str() {
                "system-design" => Some(GateKind::SystemDesign),
                "grill" | "grill-gate" => Some(GateKind::Grill),
                _ => None,
            }?;
            let status = match r.status.as_str() {
                "pass" => GateStatus::Pass,
                "fail" => GateStatus::Fail,
                "concerns" => GateStatus::Concerns,
                "missing" => GateStatus::Missing,
                _ => GateStatus::NotApplicable,
            };
            Some(forge_core_decisions::ProvidedGateResult {
                gate_kind: gk,
                status,
            })
        })
        .collect()
}

/// Emit a guide envelope to stdout (JSON) or stderr (text) and propagate
/// the envelope's exit code as an `ExitError` when non-zero.
///
/// # Errors
///
/// Returns `ExitError::with_code` carrying the envelope's non-zero exit
/// code so the entrypoint can translate it into `process::exit(code)`.
///
/// # Panics
///
/// Does NOT panic on serialization failure (V4.A). In JSON mode, if `env`
/// cannot be serialized by `serde_json`, an error is written to stderr and an
/// `ExitError::env_config` (exit code 5) is returned. `T: Serialize` is bound,
/// so this is effectively infallible in practice, but a panic is the wrong
/// tool in a shared stdout emit path.
pub fn emit_guide<T: serde::Serialize>(
    env: forge_core_contracts::CliEnvelope<T>,
    want_json: bool,
) -> Result<(), ExitError> {
    let code = env.exit_code();
    if want_json {
        // Serialize before printing so a failure is a typed error, not a panic.
        let json = serde_json::to_string_pretty(&env).map_err(|e| {
            eprintln!("internal error: failed to serialize guide envelope: {e}");
            ExitError::env_config(format!("failed to serialize guide envelope: {e}"))
        })?;
        println!("{json}");
    } else if !env.ok {
        eprintln!(
            "guide failed: {}",
            env.error.as_ref().map_or("unknown", |e| e.message.as_str())
        );
    }
    if code == 0 {
        Ok(())
    } else {
        Err(ExitError::with_code(code, String::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn tempfile_dir() -> std::path::PathBuf {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("forge-guide-test-{}-{}", std::process::id(), n));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    use forge_core_contracts::{CatalogEntry, StableId};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn assert_guide_error_projects_only_subcommand_usage(
        error: &ExitError,
        subcommand: &str,
        expected_diagnostic: &str,
    ) {
        assert_eq!(error.exit_code(), 3);
        let message = error.message();
        assert!(
            message.contains(expected_diagnostic),
            "error should preserve diagnostic {expected_diagnostic:?}: {message}"
        );
        let projected = COMMAND_GUIDE
            .usage_line_for_subcommand(subcommand)
            .expect("guide subcommand usage");
        assert!(
            message.contains(projected),
            "error should project {subcommand} Command Surface usage {projected:?}: {message}"
        );
        for sibling in ["describe", "decide", "status"] {
            if sibling != subcommand {
                let sibling_usage = COMMAND_GUIDE
                    .usage_line_for_subcommand(sibling)
                    .expect("sibling usage");
                assert!(
                    !message.contains(sibling_usage),
                    "error for {subcommand} should not leak {sibling} usage: {message}"
                );
            }
        }
    }

    fn real_catalog_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/workflows")
            .canonicalize()
            .expect("catalog dir")
    }

    #[test]
    fn describe_emits_all_110_workflows_compactly() {
        let env = run_describe(Some(&real_catalog_dir()));
        assert!(env.ok, "describe should succeed");
        assert_eq!(env.exit_code(), 0);
        let payload = env.data.as_ref().expect("payload");
        assert_eq!(payload.workflows.len(), 110);
        // every row is compact: id + phases + one summary line
        for w in &payload.workflows {
            assert!(!w.id.is_empty());
            assert!(!w.phases.is_empty());
            assert!(!w.summary.is_empty());
        }
    }

    #[test]
    fn describe_includes_phases_gates_exit_reasons_and_schema_version() {
        let env = run_describe(Some(&real_catalog_dir()));
        let p = env.data.as_ref().expect("payload");
        assert_eq!(p.schema_version, ENVELOPE_SCHEMA_VERSION);
        assert!(p.phases.contains(&"1-discovery".to_string()));
        assert!(p.phases.contains(&"6-evolve".to_string()));
        assert!(p.gates.iter().any(|g| g.gate == "system-design"));
        assert!(p.exit_reasons.contains(&"rejected_by_gate".to_string()));
    }

    #[test]
    fn describe_returns_env_config_envelope_when_catalog_dir_missing() {
        let env = run_describe(Some(std::path::Path::new("/nonexistent/does/not/exist")));
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, StableId("env_config".into()).0);
        assert_eq!(env.exit_code(), 5);
    }

    #[test]
    fn compact_workflow_uses_first_trigger_as_summary() {
        let entry = CatalogEntry {
            id: StableId("x".into()),
            phases: vec![StableId("1-discovery".into())],
            workflow_ref: forge_core_contracts::RepoPath("p".into()),
            triggers: vec!["does X, use when Y".into(), "second".into()],
            prerequisites: vec![],
            outputs: vec![],
        };
        let cw = compact_workflow(&entry);
        assert_eq!(cw.summary, "does X, use when Y");
    }

    #[test]
    fn payload_serializes_to_json_cleanly() {
        let env = run_describe(Some(&real_catalog_dir()));
        let json = serde_json::to_string(&env).expect("serialize");
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"workflows\""));
    }

    // --- guide decide tests (S3.3) ---

    use std::io::Write;
    fn write_decision(tmp: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
        let p = tmp.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    const VALID_DISCOVERY: &str = "schema_version: \"0.1\"\nguide_decision:\n  recommended_workflow: discover-intent\n  reason: start here\n  current_phase: 1-discovery\n";
    const PLAN_IN_DISCOVERY: &str = "schema_version: \"0.1\"\nguide_decision:\n  recommended_workflow: plan-sprint\n  reason: skip\n  current_phase: 1-discovery\n";
    const UNKNOWN_WF: &str = "schema_version: \"0.1\"\nguide_decision:\n  recommended_workflow: nope\n  reason: x\n  current_phase: 1-discovery\n";
    const BAD_YAML: &str = "schema_version: \"0.1\"\nguide_decision: { not valid";

    #[test]
    fn decide_accepts_valid_in_phase_decision() {
        let tmp = tempfile_dir();
        let df = write_decision(&tmp, "d.yaml", VALID_DISCOVERY);
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(env.ok, "should accept: {:?}", env.error);
        assert_eq!(env.exit_code(), 0);
        let p = env.data.as_ref().expect("payload");
        assert_eq!(p.recommended_workflow, "discover-intent");
    }

    #[test]
    fn decide_rejects_ineligible_workflow_with_typed_code() {
        let tmp = tempfile_dir();
        let df = write_decision(&tmp, "d.yaml", PLAN_IN_DISCOVERY);
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "rejected_by_gate");
        assert_eq!(env.exit_code(), 2);
        let code = env.error.as_ref().expect("error").code.0.clone();
        assert!(code.starts_with("not_eligible_in_phase"), "got: {code}");
    }

    #[test]
    fn decide_rejects_unknown_workflow() {
        let tmp = tempfile_dir();
        let df = write_decision(&tmp, "d.yaml", UNKNOWN_WF);
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(!env.ok);
        let code = env.error.as_ref().expect("error").code.0.clone();
        assert!(code.starts_with("unknown_workflow"), "got: {code}");
    }

    #[test]
    fn decide_returns_invalid_decision_shape_on_bad_yaml() {
        let tmp = tempfile_dir();
        let df = write_decision(&tmp, "d.yaml", BAD_YAML);
        let env = run_decide(&df, Some(&real_catalog_dir()), &[], None);
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "invalid_decision_shape");
        assert_eq!(env.exit_code(), 3);
    }

    #[test]
    fn decide_returns_env_config_when_decision_file_unreadable() {
        let env = run_decide(
            std::path::Path::new("/no/such/file.yaml"),
            Some(&real_catalog_dir()),
            &[],
            None,
        );
        assert!(!env.ok);
        // missing decision file = invalid input (no decision to validate) -> exit 3
        assert_eq!(env.exit_code(), 3);
    }

    // --- guide status tests (S3.4) ---

    #[test]
    fn status_reports_eligible_workflows_and_pending_gate() {
        let env = run_status(Some(&real_catalog_dir()), "2-specification");
        assert!(env.ok, "{:?}", env.error);
        let p = env.data.as_ref().expect("payload");
        assert_eq!(p.current_phase, "2-specification");
        assert!(!p.eligible_workflows.is_empty());
        // anytime workflows must be eligible in every phase
        assert!(p
            .eligible_workflows
            .iter()
            .any(|w| w.id == "adversarial-review"));
        // the system-design gate unlocks 3-plan
        assert_eq!(p.pending_gates.len(), 1);
        assert_eq!(p.pending_gates[0].gate, "system-design");
        assert_eq!(p.pending_gates[0].unlocks, "3-plan");
        assert_eq!(p.next_phases, vec!["3-plan".to_string()]);
    }

    #[test]
    fn status_rejects_unknown_phase() {
        let env = run_status(Some(&real_catalog_dir()), "nonsense");
        assert!(!env.ok);
        assert_eq!(env.exit_reason.0, "invalid_decision_shape");
        assert_eq!(env.exit_code(), 3);
    }

    #[test]
    fn status_accepts_phase_aliases() {
        // Phase::parse is permissive: "3", "plan", "3-plan" all categorize.
        for alias in ["3", "plan", "3-plan"] {
            let env = run_status(Some(&real_catalog_dir()), alias);
            assert!(env.ok, "alias '{alias}' should parse: {:?}", env.error);
            assert_eq!(env.data.as_ref().unwrap().current_phase, "3-plan");
        }
    }

    #[test]
    fn status_terminal_phase_has_no_pending_gate() {
        // evolve is the last phase; no forward gate.
        let env = run_status(Some(&real_catalog_dir()), "6-evolve");
        assert!(env.ok);
        assert!(env.data.as_ref().unwrap().pending_gates.is_empty());
    }

    #[test]
    fn guide_value_requires_present_non_flag_value() {
        let parsed: Vec<String> = ["--catalog-dir", "contracts/workflows"]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(guide_value(&parsed, 1), Some("contracts/workflows"));

        let missing: Vec<String> = ["--catalog-dir"].iter().map(ToString::to_string).collect();
        assert_eq!(guide_value(&missing, 1), None);

        let next_flag: Vec<String> = ["--catalog-dir", "--no-json"]
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(guide_value(&next_flag, 1), None);
    }

    #[test]
    fn guide_usage_projects_command_surface_lines() {
        let mut usage = String::from("forge-core guide <subcommand> [options]");
        for line in COMMAND_GUIDE.local_usage_lines() {
            usage.push('\n');
            usage.push_str("  ");
            usage.push_str(line);
        }

        assert!(
            usage.starts_with("forge-core guide <subcommand> [options]"),
            "guide usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_GUIDE.usage_lines {
            let subcommand_usage = COMMAND_GUIDE.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "guide usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(guide_subcommand_hint(), "describe | decide | status");
    }

    #[test]
    fn guide_subcommand_help_lookup_projects_full_command_surface_lines() {
        for subcommand in ["describe", "decide", "status"] {
            let usage = guide_command_surface_usage_line_for(subcommand);
            assert_eq!(
                Some(usage),
                COMMAND_GUIDE.usage_line_for_subcommand(subcommand),
                "guide {subcommand} help should come from the Command Surface"
            );
        }
    }

    #[test]
    fn guide_missing_flag_value_reports_subcommand_usage() {
        let error =
            run_guide_describe(&args(&["--catalog-dir", "--json"])).expect_err("parse error");

        assert_guide_error_projects_only_subcommand_usage(
            &error,
            "describe",
            "guide describe: --catalog-dir requires a value",
        );
    }

    #[test]
    fn guide_unknown_arg_reports_subcommand_usage() {
        let error = run_guide_status(&args(&["--bogus"])).expect_err("parse error");

        assert_guide_error_projects_only_subcommand_usage(
            &error,
            "status",
            "guide status: unrecognized argument '--bogus'",
        );
    }

    #[test]
    fn guide_missing_required_flags_report_subcommand_usage() {
        let decide_error = run_guide_decide(&args(&[])).expect_err("missing decision file");
        assert_guide_error_projects_only_subcommand_usage(
            &decide_error,
            "decide",
            "guide decide: --decision-file is required",
        );

        // `guide status` without --phase no longer errors: it falls back to
        // the authoritative `state.yaml` phase (or `1-discovery` when no root
        // is provided). Only `--decision-file` is required on `decide`.
        run_guide_status(&args(&[])).expect("status falls back to 1-discovery");
    }

    #[test]
    fn guide_status_accepts_explicit_json_mode() {
        let status_args = args(&[
            "--json",
            "--phase",
            "3-plan",
            "--catalog-dir",
            real_catalog_dir().to_str().expect("catalog path utf-8"),
        ]);
        let result = run_guide_status(&status_args);

        assert!(result.is_ok(), "explicit --json should parse");
    }
}
