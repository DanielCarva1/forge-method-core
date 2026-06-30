//! `forge preflight` — unified readiness gate.
//!
//! Runs every required gate (cargo check / clippy pedantic / test / fmt /
//! validate contracts / regression anchor) and reports the combined status.
//! Optional gates (cargo bench, fuzz smoke) can be enabled with flags but do
//! not fail the run by default.
//!
//! Design goals (per F02 in the Excellence Roadmap):
//! - Single command that answers "is this branch shippable?"
//! - Each gate has typed status, duration, and diagnostics.
//! - Accumulating: a failed required gate does not skip the remaining gates.
//! - Fail-soft: unknown gates and optional-gate errors degrade the run to
//!   `Degraded` instead of `Failed`.
//! - JSON output is stable and machine-readable so agents and CI can parse it.

use crate::cli_error::ExitError;
use serde::Serialize;
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
            Self::ConflictingFlags { left, right } => {
                write!(formatter, "conflicting flags: {left} and {right}")
            }
        }
    }
}

impl std::error::Error for PreflightArgError {}

/// Whether a gate must pass for the run to be considered ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
    pub gate: GateKind,
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
    pub root: std::path::PathBuf,
    pub allow_bootstrap_core: bool,
    pub json: bool,
    /// When empty, runs all default gates. When non-empty, runs only the named
    /// gates (still classified by their default requirement).
    pub gates: Vec<GateKind>,
    /// Override the anchor count (default 122 for the forge repo).
    pub expected_anchor: usize,
}

impl Default for PreflightInput {
    fn default() -> Self {
        Self {
            root: std::path::PathBuf::from("."),
            allow_bootstrap_core: false,
            json: false,
            gates: Vec::new(),
            expected_anchor: 122,
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
    // `args[0]` is the command name ("preflight"); skip it for parsing.
    let tail = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let input = parse_preflight_args(&tail).map_err(|error| {
        ExitError::usage(format!(
            "{}: {error}",
            preflight_usage()
        ))
    })?;
    let report = run_preflight(&input);
    if input.json {
        let serialized = serde_json::to_string_pretty(&report)
            .expect("preflight report is always serializable");
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
                input.expected_anchor = value
                    .parse()
                    .map_err(|_| PreflightArgError::InvalidGate {
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
    let gates_to_run: Vec<GateKind> = if input.gates.is_empty() {
        GateKind::default_required().to_vec()
    } else {
        input.gates.clone()
    };
    let mut results = Vec::with_capacity(gates_to_run.len());
    for gate in gates_to_run {
        results.push(run_gate(gate, GateRequirement::Required, input));
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

#[instrument(skip_all, fields(gate = gate.wire_name()), level = "info")]
fn run_gate(
    gate: GateKind,
    requirement: GateRequirement,
    input: &PreflightInput,
) -> GateResult {
    let started = Instant::now();
    let outcome = execute_gate(gate, input);
    let duration_ms = started.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let (status, log_tail) = match outcome {
        GateOutcome::Passed => (GateStatus::Passed, Vec::new()),
        GateOutcome::Failed { log_tail } => (GateStatus::Failed, log_tail),
    };
    GateResult {
        gate,
        requirement,
        status,
        duration_ms,
        log_tail,
    }
}

enum GateOutcome {
    Passed,
    Failed { log_tail: Vec<String> },
}

// `Skipped` is intentionally not modeled here: the preflight runs only the
// gates selected by the user. Optional gates (cargo bench, fuzz smoke) will
// be added as separate `GateKind` variants with `GateRequirement::Optional`.

fn execute_gate(gate: GateKind, input: &PreflightInput) -> GateOutcome {
    match gate {
        GateKind::TypeCheck => run_cargo_gate(&[
            "check",
            "--workspace",
            "--all-targets",
        ]),
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
        println!(
            "  {tag} {:<18} {:>6} ms",
            gate.gate.wire_name(),
            gate.duration_ms,
        );
        for line in &gate.log_tail {
            println!("        {line}");
        }
    }
}

/// Static usage string. Used by `main.rs` and `--help`.
#[must_use]
pub fn preflight_usage() -> &'static str {
    "usage: forge-core preflight [--root <path>] [--allow-bootstrap-core] [--json|--no-json] [--gate <name>]... [--expected-anchor <count>]\n  gates: type_check, clippy_pedantic, test, format, validate, regression_anchor"
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
        let error = parse_preflight_args(&[
            "--gate".to_string(),
            "lint".to_string(),
        ])
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
        assert!(matches!(
            error,
            PreflightArgError::ConflictingFlags { .. }
        ));
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
                gate: GateKind::TypeCheck,
                requirement: GateRequirement::Required,
                status: GateStatus::Passed,
                duration_ms: 100,
                log_tail: Vec::new(),
            },
            GateResult {
                gate: GateKind::Test,
                requirement: GateRequirement::Required,
                status: GateStatus::Failed,
                duration_ms: 200,
                log_tail: Vec::new(),
            },
            GateResult {
                gate: GateKind::Format,
                requirement: GateRequirement::Optional,
                status: GateStatus::Failed,
                duration_ms: 50,
                log_tail: Vec::new(),
            },
            GateResult {
                gate: GateKind::Validate,
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
