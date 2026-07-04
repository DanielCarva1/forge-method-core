//! `forge preflight` — unified readiness gate.
//!
//! Runs every required gate for the project's profile and reports the combined
//! status. The profile is auto-detected from the project root (Cargo.toml →
//! rust, package.json → node, …) unless overridden by `--profile` or pinned by
//! a `.forge-method/preflight.yaml` document written by `preflight init`.
//!
//! Built-in gates:
//! - **Rust profile** runs the four cargo gates (`type_check`, `format`,
//!   `clippy_pedantic`, `test`) plus the language-agnostic `validate` and
//!   `regression_anchor`.
//! - **Other profiles** (node/python/go/generic) run only `validate` and
//!   `regression_anchor`. Cargo gates under these profiles are `Skipped`, not
//!   `Failed` — so a non-Rust project never reports a misleading
//!   "Cargo.toml not found" error.
//!
//! Custom gates: the profile document may declare shell-command gates (e.g.
//! `["npm","test"]`, `["pytest"]`, `["npx","@bruno/api"]`) whose exit code is
//! the verdict. This mirrors pre-commit's `language: system` hooks and lets
//! any project encode its real readiness checks without Rust code in the
//! core.
//!
//! Design goals (per F02 in the Excellence Roadmap):
//! - Single command that answers "is this branch shippable?" for any project.
//! - Each gate has typed status, duration, and diagnostics.
//! - Accumulating: a failed required gate does not skip the remaining gates.
//! - Fail-soft: unknown gates and optional-gate errors degrade the run to
//!   `Degraded` instead of `Failed`.
//! - Profile-aware: cargo gates skip silently when the profile is not Rust.
//! - JSON output is stable and machine-readable so agents and CI can parse it.

use crate::cli_error::ExitError;
use crate::project_profile::{
    PreflightProfileDocument, PREFLIGHT_PROFILE_FILE_NAME, PREFLIGHT_PROFILE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::instrument;

/// Overall status of a preflight run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightStatus {
    /// Every required gate passed; optional gates either passed or were skipped.
    Ready,
    /// Every required gate passed but at least one optional gate failed.
    Degraded,
    /// At least one required gate failed.
    Failed,
}

/// Status of a single gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Passed,
    Failed,
    Skipped,
}

/// Hand-rolled error enum for argument parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightArgError {
    UnknownArgument { argument: String },
    InvalidGate { value: String },
    InvalidProfile { value: String },
    ConflictingFlags { left: String, right: String },
}

impl std::fmt::Display for PreflightArgError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownArgument { argument } => {
                write!(formatter, "unknown argument: {argument}")
            }
            Self::InvalidGate { value } => {
                write!(formatter, "invalid gate name: {value}")
            }
            Self::InvalidProfile { value } => {
                write!(formatter, "invalid profile name: {value}")
            }
            Self::ConflictingFlags { left, right } => {
                write!(formatter, "conflicting flags: {left} and {right}")
            }
        }
    }
}

impl std::error::Error for PreflightArgError {}

/// Whether a gate must pass for the run to be considered ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateRequirement {
    Required,
    Optional,
}

/// Identifier for a known gate. Wire names are stable and part of the public
/// CLI contract; agents and CI may pin to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    /// `cargo check --workspace --all-targets`
    TypeCheck,
    /// `cargo clippy --workspace --all-targets -- -W clippy::pedantic -D warnings`
    ClippyPedantic,
    /// `cargo test --workspace`
    Test,
    /// `cargo fmt --all -- --check`
    Format,
    /// `forge validate --root <root>`
    Validate,
    /// `forge validate --root <root> --json | grep -c '"diagnostics": 0'` equals the anchor.
    RegressionAnchor,
}

impl GateKind {
    /// All gates that run by default, in execution order.
    #[must_use]
    pub fn default_required() -> &'static [GateKind] {
        &[
            GateKind::TypeCheck,
            GateKind::Format,
            GateKind::ClippyPedantic,
            GateKind::Test,
            GateKind::Validate,
            GateKind::RegressionAnchor,
        ]
    }

    /// Stable wire name used by `--gate` and JSON output.
    #[must_use]
    pub fn wire_name(self) -> &'static str {
        match self {
            GateKind::TypeCheck => "type_check",
            GateKind::ClippyPedantic => "clippy_pedantic",
            GateKind::Test => "test",
            GateKind::Format => "format",
            GateKind::Validate => "validate",
            GateKind::RegressionAnchor => "regression_anchor",
        }
    }

    /// Parse a wire name back into a variant. Returns `None` for unknown names.
    #[must_use]
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "type_check" => Some(Self::TypeCheck),
            "clippy_pedantic" => Some(Self::ClippyPedantic),
            "test" => Some(Self::Test),
            "format" => Some(Self::Format),
            "validate" => Some(Self::Validate),
            "regression_anchor" => Some(Self::RegressionAnchor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    /// Gate name. For built-ins this is a [`GateKind`] wire name
    /// (`type_check`, `validate`, ...); for custom gates it is whatever the
    /// profile document declared.
    pub name: String,
    pub requirement: GateRequirement,
    pub status: GateStatus,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Tail of the command's combined stdout+stderr, truncated to a reasonable
    /// bound so JSON output stays compact. Empty when the gate passed cleanly.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub log_tail: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightReport {
    pub overall_status: PreflightStatus,
    pub run_unix: u64,
    /// Combined wall-clock duration of the preflight run in milliseconds.
    pub duration_ms: u64,
    pub gates: Vec<GateResult>,
    pub summary: PreflightSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub required_failed: usize,
    pub optional_failed: usize,
}

#[derive(Debug, Clone)]
pub struct PreflightInput {
    pub root: PathBuf,
    pub allow_bootstrap_core: bool,
    pub json: bool,
    /// When empty, runs all default gates. When non-empty, runs only the named
    /// gates (still classified by their default requirement).
    pub gates: Vec<GateKind>,
    /// Override the anchor count (default 122 for the forge repo).
    pub expected_anchor: usize,
    /// Force a profile instead of auto-detecting. `None` means auto-detect
    /// from the project root (and read `.forge-method/preflight.yaml` if
    /// present).
    pub profile_override: Option<crate::project_profile::ProjectProfile>,
}

impl Default for PreflightInput {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            allow_bootstrap_core: false,
            json: false,
            gates: Vec::new(),
            expected_anchor: 122,
            profile_override: None,
        }
    }
}

const LOG_TAIL_MAX_LINES: usize = 25;

/// Entry point for `forge preflight`. Dispatches to argument parsing, runs the
/// gates, and prints either JSON or a human-readable summary.
///
/// # Errors
///
/// Returns [`ExitError::usage`] on argument parsing errors. Returns
/// [`ExitError::failed`] only when an internal invariant is violated (e.g
/// serialization failure). Gate failures do NOT produce `Err`: they are
/// surfaced via the report's `overall_status = Failed`.
#[allow(clippy::missing_panics_doc)] // serialisation is infallible on this type
pub fn run_preflight_command(args: &[String]) -> Result<(), ExitError> {
    // `args[0]` is the command name ("preflight"). A second token may be a
    // subcommand (`init`) rather than a flag; dispatch it first.
    let sub = args.get(1).map(String::as_str);
    if sub == Some("init") {
        return run_preflight_init_command(&args[2..]);
    }
    // Otherwise treat the tail as the preflight run's flag list.
    let tail = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let input = parse_preflight_args(&tail)
        .map_err(|error| ExitError::usage(format!("{}: {error}", preflight_usage())))?;
    let report = run_preflight(&input);
    if input.json {
        let serialized =
            serde_json::to_string_pretty(&report).expect("preflight report is always serializable");
        println!("{serialized}");
    } else {
        print_human_summary(&report);
    }
    // Non-zero exit only when a required gate failed. Degraded still exits 0
    // so CI pipelines can decide whether to tolerate optional-gate failures.
    match report.overall_status {
        PreflightStatus::Failed => Err(ExitError::failed("preflight failed")),
        PreflightStatus::Degraded | PreflightStatus::Ready => Ok(()),
    }
}

/// Parse preflight CLI arguments.
///
/// # Errors
///
/// Returns [`PreflightArgError`] for unknown arguments, invalid gate names, or
/// conflicting flags.
pub fn parse_preflight_args(args: &[String]) -> Result<PreflightInput, PreflightArgError> {
    let mut input = PreflightInput::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let Some(value) = args.next() else {
                    return Err(PreflightArgError::UnknownArgument {
                        argument: "--root <missing value>".to_string(),
                    });
                };
                input.root = std::path::PathBuf::from(value);
            }
            "--allow-bootstrap-core" => input.allow_bootstrap_core = true,
            "--json" => input.json = true,
            "--no-json" => input.json = false,
            "--profile" => {
                let Some(value) = args.next() else {
                    return Err(PreflightArgError::UnknownArgument {
                        argument: "--profile <missing value>".to_string(),
                    });
                };
                let profile = crate::project_profile::ProjectProfile::from_wire_name(value)
                    .ok_or_else(|| PreflightArgError::InvalidProfile {
                        value: value.clone(),
                    })?;
                if input.profile_override.is_some() {
                    return Err(PreflightArgError::ConflictingFlags {
                        left: format!("--profile {value}"),
                        right: "duplicate --profile".to_string(),
                    });
                }
                input.profile_override = Some(profile);
            }
            "--gate" => {
                let Some(value) = args.next() else {
                    return Err(PreflightArgError::UnknownArgument {
                        argument: "--gate <missing value>".to_string(),
                    });
                };
                let gate = GateKind::from_wire_name(value).ok_or_else(|| {
                    PreflightArgError::InvalidGate {
                        value: value.clone(),
                    }
                })?;
                if input.gates.contains(&gate) {
                    return Err(PreflightArgError::ConflictingFlags {
                        left: format!("--gate {value}"),
                        right: "duplicate --gate".to_string(),
                    });
                }
                input.gates.push(gate);
            }
            "--expected-anchor" => {
                let Some(value) = args.next() else {
                    return Err(PreflightArgError::UnknownArgument {
                        argument: "--expected-anchor <missing value>".to_string(),
                    });
                };
                input.expected_anchor =
                    value.parse().map_err(|_| PreflightArgError::InvalidGate {
                        value: format!("--expected-anchor {value}"),
                    })?;
            }
            "--help" | "-h" => {
                return Err(PreflightArgError::UnknownArgument {
                    argument: "help".to_string(),
                });
            }
            other => {
                return Err(PreflightArgError::UnknownArgument {
                    argument: other.to_string(),
                });
            }
        }
    }
    Ok(input)
}

#[instrument(skip_all, level = "info")]
pub fn run_preflight(input: &PreflightInput) -> PreflightReport {
    let run_started = Instant::now();
    let run_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    // Resolve the effective profile: explicit `--profile` flag wins, then the
    // pinned `.forge-method/preflight.yaml`, then auto-detection from the
    // project root. The resolved profile plus its gate set drive the run.
    let profile = resolve_profile(input);
    let specs = resolve_gate_specs(input, profile);

    let mut results = Vec::with_capacity(specs.len());
    for spec in specs {
        results.push(run_gate_spec(&spec, profile, input));
    }
    let elapsed = run_started.elapsed();
    let duration_ms = elapsed.as_secs() * 1_000 + u64::from(elapsed.subsec_millis());
    let summary = summarize(&results);
    let overall_status = if summary.required_failed > 0 {
        PreflightStatus::Failed
    } else if summary.optional_failed > 0 {
        PreflightStatus::Degraded
    } else {
        PreflightStatus::Ready
    };
    PreflightReport {
        overall_status,
        run_unix,
        duration_ms,
        gates: results,
        summary,
    }
}

/// Decide which profile governs this run.
#[must_use]
pub fn resolve_profile(input: &PreflightInput) -> crate::project_profile::ProjectProfile {
    if let Some(forced) = input.profile_override {
        return forced;
    }
    if let Some(doc) = read_profile_document(&input.root) {
        return doc.profile;
    }
    crate::project_profile::ProjectProfile::detect(&input.root)
}

/// Decide which gates to run. Explicit `--gate` flags on the CLI select
/// built-in gates by name (legacy behaviour, always Required). Otherwise the
/// profile document's gate list wins, falling back to the profile defaults.
#[must_use]
pub fn resolve_gate_specs(
    input: &PreflightInput,
    profile: crate::project_profile::ProjectProfile,
) -> Vec<crate::project_profile::GateSpec> {
    // Legacy `--gate` path: built-in gate names selected on the CLI. These
    // always run as Required and bypass the profile document entirely, so a
    // Rust developer can still `forge-core preflight --gate format` on a
    // repo without a profile file.
    if !input.gates.is_empty() {
        return input
            .gates
            .iter()
            .map(|kind| crate::project_profile::GateSpec::builtin(*kind, GateRequirement::Required))
            .collect();
    }
    if let Some(doc) = read_profile_document(&input.root) {
        if !doc.gates.is_empty() {
            return doc.gates;
        }
    }
    profile.default_gates()
}

/// Read the profile document from `<root>/.forge-method/preflight.yaml`, if
/// it exists. Returns `None` silently when the file is absent (the common
/// case). A malformed file is also treated as `None` so that a bad profile
/// degrades to auto-detection rather than failing the whole preflight.
fn read_profile_document(root: &Path) -> Option<PreflightProfileDocument> {
    let path = root.join(".forge-method").join(PREFLIGHT_PROFILE_FILE_NAME);
    let raw = std::fs::read_to_string(&path).ok()?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let doc: PreflightProfileDocument = yaml_serde::from_str(raw).ok()?;
    // schema_version sanity: accept the current version, ignore future/foreign.
    if doc.schema_version != PREFLIGHT_PROFILE_SCHEMA_VERSION {
        return None;
    }
    Some(doc)
}

/// Run a single [`GateSpec`] (built-in or custom) and produce a result.
fn run_gate_spec(
    spec: &crate::project_profile::GateSpec,
    profile: crate::project_profile::ProjectProfile,
    input: &PreflightInput,
) -> GateResult {
    let name = spec.name.clone();
    let started = Instant::now();
    let outcome = if spec.is_builtin() {
        execute_builtin_gate(&spec.name, profile, input)
    } else {
        execute_custom_gate(spec, &input.root)
    };
    let duration_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let (status, log_tail) = match outcome {
        GateOutcome::Passed => (GateStatus::Passed, Vec::new()),
        GateOutcome::Skipped => (GateStatus::Skipped, Vec::new()),
        GateOutcome::Failed { log_tail } => (GateStatus::Failed, log_tail),
    };
    GateResult {
        name,
        requirement: spec.requirement,
        status,
        duration_ms,
        log_tail,
    }
}

#[derive(Debug)]
pub enum GateOutcome {
    Passed,
    Skipped,
    Failed { log_tail: Vec<String> },
}

/// Execute a built-in gate by its canonical wire name. Cargo gates only apply
/// to the Rust profile; under any other profile they are `Skipped` (not
/// `Failed`), so a Node/Python/Generic project never reports a misleading
/// "Cargo.toml not found" failure.
#[must_use]
pub fn execute_builtin_gate(
    name: &str,
    profile: crate::project_profile::ProjectProfile,
    input: &PreflightInput,
) -> GateOutcome {
    let Some(kind) = GateKind::from_wire_name(name) else {
        return GateOutcome::Skipped;
    };
    // Cargo gates are meaningful only under the Rust profile. Any other
    // profile skips them rather than running cargo against a project that has
    // no Cargo.toml (the original bug).
    if matches!(
        kind,
        GateKind::TypeCheck | GateKind::ClippyPedantic | GateKind::Test | GateKind::Format
    ) && profile != crate::project_profile::ProjectProfile::Rust
    {
        return GateOutcome::Skipped;
    }
    match kind {
        GateKind::TypeCheck => run_cargo_gate(&["check", "--workspace", "--all-targets"]),
        GateKind::ClippyPedantic => run_cargo_gate(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-W",
            "clippy::pedantic",
            "-D",
            "warnings",
        ]),
        GateKind::Test => run_cargo_gate(&["test", "--workspace"]),
        GateKind::Format => run_cargo_gate(&["fmt", "--all", "--", "--check"]),
        GateKind::Validate => run_validate_gate(input),
        GateKind::RegressionAnchor => run_anchor_gate(input),
    }
}

/// Execute a custom shell gate. The command runs with the project root as its
/// working directory; exit code 0 means pass, non-zero means fail. This
/// mirrors how pre-commit's `language: system` hooks and CI runners treat
/// arbitrary commands.
fn execute_custom_gate(spec: &crate::project_profile::GateSpec, root: &Path) -> GateOutcome {
    let Some((program, args)) = spec.command.split_first() else {
        return GateOutcome::Skipped;
    };
    let output = Command::new(program).args(args).current_dir(root).output();
    match output {
        Ok(output) if output.status.success() => GateOutcome::Passed,
        Ok(output) => GateOutcome::Failed {
            log_tail: tail_combined_output(&output),
        },
        Err(error) => GateOutcome::Failed {
            log_tail: vec![format!(
                "custom gate '{}' failed to spawn: {error}",
                spec.name
            )],
        },
    }
}

fn run_cargo_gate(args: &[&str]) -> GateOutcome {
    let output = Command::new("cargo").args(args).output();
    match output {
        Ok(output) if output.status.success() => GateOutcome::Passed,
        Ok(output) => GateOutcome::Failed {
            log_tail: tail_combined_output(&output),
        },
        Err(error) => GateOutcome::Failed {
            log_tail: vec![format!("cargo invocation failed: {error}")],
        },
    }
}

fn run_validate_gate(input: &PreflightInput) -> GateOutcome {
    let mut command = Command::new("cargo");
    command.args(["run", "--quiet", "-p", "forge-core-cli", "--", "validate"]);
    command.arg("--root").arg(&input.root);
    if input.allow_bootstrap_core {
        command.arg("--allow-bootstrap-core");
    }
    let output = command.output();
    match output {
        Ok(output) if output.status.success() => GateOutcome::Passed,
        Ok(output) => GateOutcome::Failed {
            log_tail: tail_combined_output(&output),
        },
        Err(error) => GateOutcome::Failed {
            log_tail: vec![format!("validate invocation failed: {error}")],
        },
    }
}

fn run_anchor_gate(input: &PreflightInput) -> GateOutcome {
    let mut command = Command::new("cargo");
    command.args([
        "run",
        "--quiet",
        "-p",
        "forge-core-cli",
        "--",
        "validate",
        "--root",
    ]);
    command.arg(&input.root);
    command.arg("--json");
    if input.allow_bootstrap_core {
        command.arg("--allow-bootstrap-core");
    }
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            return GateOutcome::Failed {
                log_tail: vec![format!("anchor invocation failed: {error}")],
            };
        }
    };
    if !output.status.success() {
        return GateOutcome::Failed {
            log_tail: tail_combined_output(&output),
        };
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let count = stdout.matches("\"diagnostics\": 0").count();
    if count == input.expected_anchor {
        GateOutcome::Passed
    } else {
        GateOutcome::Failed {
            log_tail: vec![format!(
                "anchor count {count} != expected {}",
                input.expected_anchor
            )],
        }
    }
}

fn tail_combined_output(output: &std::process::Output) -> Vec<String> {
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str("\n--- stderr ---\n");
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined
        .lines()
        .rev()
        .take(LOG_TAIL_MAX_LINES)
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn summarize(results: &[GateResult]) -> PreflightSummary {
    let mut summary = PreflightSummary {
        total: results.len(),
        passed: 0,
        failed: 0,
        skipped: 0,
        required_failed: 0,
        optional_failed: 0,
    };
    for result in results {
        match result.status {
            GateStatus::Passed => summary.passed += 1,
            GateStatus::Failed => {
                summary.failed += 1;
                match result.requirement {
                    GateRequirement::Required => summary.required_failed += 1,
                    GateRequirement::Optional => summary.optional_failed += 1,
                }
            }
            GateStatus::Skipped => summary.skipped += 1,
        }
    }
    summary
}

fn print_human_summary(report: &PreflightReport) {
    println!(
        "forge preflight: {} ({} of {} gates passed, {} failed, {} skipped) in {} ms",
        match report.overall_status {
            PreflightStatus::Ready => "READY",
            PreflightStatus::Degraded => "DEGRADED",
            PreflightStatus::Failed => "FAILED",
        },
        report.summary.passed,
        report.summary.total,
        report.summary.failed,
        report.summary.skipped,
        report.duration_ms,
    );
    for gate in &report.gates {
        let tag = match (gate.requirement, gate.status) {
            (GateRequirement::Required, GateStatus::Passed) => "[req ][ pass]",
            (GateRequirement::Required, GateStatus::Failed) => "[req ][FAIL ]",
            (GateRequirement::Required, GateStatus::Skipped) => "[req ][skip ]",
            (GateRequirement::Optional, GateStatus::Passed) => "[opt  ][ pass]",
            (GateRequirement::Optional, GateStatus::Failed) => "[opt  ][FAIL ]",
            (GateRequirement::Optional, GateStatus::Skipped) => "[opt  ][skip ]",
        };
        println!("  {tag} {:<18} {:>6} ms", gate.name, gate.duration_ms);
        for line in &gate.log_tail {
            println!("        {line}");
        }
    }
}

/// Entry point for `forge-core preflight init`. Detects the project profile
/// and writes `.forge-method/preflight.yaml` with the default gate set for
/// that profile. The agent calls this during onboarding so subsequent
/// `preflight` runs are deterministic; a human never needs to edit anything
/// by hand for the common case.
///
/// # Errors
///
/// Returns [`ExitError::usage`] on argument errors, [`ExitError::failed`] on
/// I/O or serialization failure.
pub fn run_preflight_init_command(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut profile_override = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                let Some(value) = args.next() else {
                    return Err(ExitError::usage(format!(
                        "{}: --root <missing value>",
                        preflight_init_usage()
                    )));
                };
                root = PathBuf::from(value);
            }
            "--profile" => {
                let Some(value) = args.next() else {
                    return Err(ExitError::usage(format!(
                        "{}: --profile <missing value>",
                        preflight_init_usage()
                    )));
                };
                let profile = crate::project_profile::ProjectProfile::from_wire_name(value)
                    .ok_or_else(|| {
                        ExitError::usage(format!(
                            "{}: invalid profile '{value}'",
                            preflight_init_usage()
                        ))
                    })?;
                profile_override = Some(profile);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", preflight_init_usage());
                return Ok(());
            }
            other => {
                return Err(ExitError::usage(format!(
                    "{}: unknown argument '{other}'",
                    preflight_init_usage()
                )));
            }
        }
    }
    let canonical = canonicalize_root(&root)?;
    let profile = profile_override
        .unwrap_or_else(|| crate::project_profile::ProjectProfile::detect(&canonical));
    let doc = PreflightProfileDocument::for_detected_profile(profile);
    let sidecar = canonical.join(".forge-method");
    std::fs::create_dir_all(&sidecar).map_err(|e| {
        ExitError::failed(format!(
            "preflight init: cannot create {}: {e}",
            sidecar.display()
        ))
    })?;
    let path = sidecar.join(PREFLIGHT_PROFILE_FILE_NAME);
    let yaml = yaml_serde::to_string(&doc)
        .map_err(|e| ExitError::failed(format!("preflight init: cannot serialize profile: {e}")))?;
    std::fs::write(&path, yaml).map_err(|e| {
        ExitError::failed(format!(
            "preflight init: cannot write {}: {e}",
            path.display()
        ))
    })?;
    let gate_names: Vec<&str> = doc.gates.iter().map(|g| g.name.as_str()).collect();
    if json {
        let payload = serde_json::json!({
            "ok": true,
            "profile": profile.wire_name(),
            "path": path.display().to_string(),
            "gates": gate_names,
        });
        let serialized = serde_json::to_string_pretty(&payload).map_err(|e| {
            ExitError::failed(format!("preflight init: cannot serialize output: {e}"))
        })?;
        println!("{serialized}");
    } else {
        println!(
            "preflight init: wrote {} (profile={}, gates=[{}])",
            path.display(),
            profile.wire_name(),
            gate_names.join(", ")
        );
    }
    Ok(())
}

/// Canonicalize a `--root` argument, falling back to the literal path when
/// canonicalization fails (e.g. a non-existent path on Windows).
fn canonicalize_root(root: &Path) -> Result<PathBuf, ExitError> {
    std::fs::canonicalize(root).or_else(|_| {
        root.canonicalize().map_err(|e| {
            ExitError::failed(format!(
                "preflight: --root {} is invalid: {e}",
                root.display()
            ))
        })
    })
}

/// Usage for the `preflight init` subcommand.
#[must_use]
pub fn preflight_init_usage() -> &'static str {
    "usage: forge-core preflight init [--root <path>] [--profile <name>] [--json|--no-json]\n  profiles: rust, node, python, go, generic"
}

/// Static usage string. Used by `main.rs` and `--help`.
#[must_use]
pub fn preflight_usage() -> &'static str {
    "usage: forge-core preflight [--root <path>] [--allow-bootstrap-core] [--json|--no-json] [--profile <name>] [--gate <name>]... [--expected-anchor <count>]\n       forge-core preflight init [--root <path>] [--profile <name>] [--json|--no-json]\n  profiles: rust, node, python, go, generic\n  gates: type_check, clippy_pedantic, test, format, validate, regression_anchor"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_preflight_args_defaults() {
        let input = parse_preflight_args(&[]).expect("empty argv");
        assert_eq!(input.root, std::path::PathBuf::from("."));
        assert!(!input.allow_bootstrap_core);
        assert!(!input.json);
        assert!(input.gates.is_empty());
        assert_eq!(input.expected_anchor, 122);
    }

    #[test]
    fn parse_preflight_args_rejects_unknown_argument() {
        let error = parse_preflight_args(&["--bogus".to_string()])
            .expect_err("unknown argument should error");
        assert!(matches!(
            error,
            PreflightArgError::UnknownArgument { argument }
            if argument == "--bogus"
        ));
    }

    #[test]
    fn parse_preflight_args_parses_known_gate() {
        let input = parse_preflight_args(&[
            "--gate".to_string(),
            "format".to_string(),
            "--gate".to_string(),
            "test".to_string(),
        ])
        .expect("known gates");
        assert_eq!(input.gates, vec![GateKind::Format, GateKind::Test]);
    }

    #[test]
    fn parse_preflight_args_rejects_invalid_gate() {
        let error = parse_preflight_args(&["--gate".to_string(), "lint".to_string()])
            .expect_err("invalid gate should error");
        assert!(matches!(
            error,
            PreflightArgError::InvalidGate { value }
            if value == "lint"
        ));
    }

    #[test]
    fn parse_preflight_args_rejects_duplicate_gate() {
        let error = parse_preflight_args(&[
            "--gate".to_string(),
            "format".to_string(),
            "--gate".to_string(),
            "format".to_string(),
        ])
        .expect_err("duplicate gate should error");
        assert!(matches!(error, PreflightArgError::ConflictingFlags { .. }));
    }

    #[test]
    fn gate_kind_wire_round_trip() {
        for gate in [
            GateKind::TypeCheck,
            GateKind::ClippyPedantic,
            GateKind::Test,
            GateKind::Format,
            GateKind::Validate,
            GateKind::RegressionAnchor,
        ] {
            assert_eq!(
                GateKind::from_wire_name(gate.wire_name()),
                Some(gate),
                "wire name round-trip failed for {gate:?}"
            );
        }
        assert_eq!(GateKind::from_wire_name("nonsense"), None);
    }

    #[test]
    fn summarize_counts_requirements_correctly() {
        let results = vec![
            GateResult {
                name: "type_check".to_string(),
                requirement: GateRequirement::Required,
                status: GateStatus::Passed,
                duration_ms: 100,
                log_tail: Vec::new(),
            },
            GateResult {
                name: "test".to_string(),
                requirement: GateRequirement::Required,
                status: GateStatus::Failed,
                duration_ms: 200,
                log_tail: Vec::new(),
            },
            GateResult {
                name: "format".to_string(),
                requirement: GateRequirement::Optional,
                status: GateStatus::Failed,
                duration_ms: 50,
                log_tail: Vec::new(),
            },
            GateResult {
                name: "validate".to_string(),
                requirement: GateRequirement::Required,
                status: GateStatus::Skipped,
                duration_ms: 0,
                log_tail: Vec::new(),
            },
        ];
        let summary = summarize(&results);
        assert_eq!(summary.total, 4);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 2);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.required_failed, 1);
        assert_eq!(summary.optional_failed, 1);
    }
}
