//! `forge-core risk-audit` — F11 Risk Audit Gate (standalone CLI).
//!
//! Loads a `risk-audit-v0` rule set from `--rules <path>`, walks the
//! consumer repo at `--root`, and runs the rules against every source file
//! whose path matches an `applies_to` glob. Findings accumulate into a
//! `ValidationReport` and the command fails closed when the report has any
//! `Error`-severity diagnostic.
//!
//! The file walker lives in `forge-core-validate::risk_audit::collect_risk_audit_targets`
//! (V3.A moved it there so the same walker backs both this standalone command
//! and the risk-audit mutation gate in the kernel). It visits every regular
//! file under `--root`, skipping `.git`, `target`, `node_modules`, and
//! `.forge-method`. Future sub-tracks can extend the walker (gitignore
//! awareness, symlinks, etc.) without touching the rule engine.

use crate::cli_error::ExitError;
use crate::cli_util::{resolve_now_unix, usage};
use forge_core_contracts::{CliEnvelope, ExitReason};
use forge_core_store::append_trace_event;
use forge_core_validate::risk_audit::{
    collect_risk_audit_targets, evaluate_risk_audit, validate_risk_audit_rule_set, RiskAuditRuleSet,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::instrument;

const RISK_AUDIT_USAGE_LINE: &str =
    "       forge-core risk-audit [--root <path>] --rules <path> [--json]";

#[derive(Debug, Clone, Serialize)]
struct RiskAuditDiagnosticView {
    severity: String,
    code: String,
    path: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct RiskAuditSummary {
    rule_count: usize,
    target_count: usize,
    diagnostic_count: usize,
    error_count: usize,
    warning_count: usize,
    diagnostics: Vec<RiskAuditDiagnosticView>,
}

impl RiskAuditSummary {
    fn passed(&self) -> bool {
        self.error_count == 0
    }
}

#[derive(Debug, Clone)]
enum RiskAuditError {
    MissingRules,
    RulesUnreadable { path: String, source: String },
    RulesParseFailed { source: String },
    RuleSetInvalid { first_error: String },
}

impl RiskAuditError {
    fn exit_reason(&self) -> ExitReason {
        match self {
            // Missing/unreadable rules is an environment/config problem.
            Self::MissingRules | Self::RulesUnreadable { .. } => ExitReason::EnvConfig,
            // Malformed rules YAML or structurally invalid rule set is an
            // input-shape problem the caller can fix.
            Self::RulesParseFailed { .. } | Self::RuleSetInvalid { .. } => {
                ExitReason::InvalidDecisionShape
            }
        }
    }
}

impl std::fmt::Display for RiskAuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRules => write!(f, "risk-audit requires --rules <path>"),
            Self::RulesUnreadable { path, source } => {
                write!(f, "could not read rules file `{path}`: {source}")
            }
            Self::RulesParseFailed { source } => {
                write!(
                    f,
                    "could not parse rules file as risk-audit-v0 YAML: {source}"
                )
            }
            Self::RuleSetInvalid { first_error } => {
                write!(f, "rule set is structurally invalid: {first_error}")
            }
        }
    }
}

/// Run the `risk-audit` CLI command.
///
/// # Errors
///
/// Returns `ExitError` when the argv shape is invalid (missing flag values)
/// or when the underlying command fails to read the rule set / walk the
/// repository / serialize the resulting envelope.
///
/// # Panics
///
/// Panics only if the JSON serializer for the command envelope fails, which
/// is impossible for the well-formed `RiskAuditSummary` types we emit.
#[instrument(skip_all, fields(root = tracing::field::Empty, rules = tracing::field::Empty, json = tracing::field::Empty, target_count = tracing::field::Empty, diagnostic_count = tracing::field::Empty), level = "info")]
pub fn run_risk_audit_command(args: &[String]) -> Result<(), ExitError> {
    let mut root = PathBuf::from(".");
    let mut rules_path: Option<PathBuf> = None;
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage(usage()));
                };
                root = PathBuf::from(value);
            }
            "--rules" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage(usage()));
                };
                rules_path = Some(PathBuf::from(value));
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{RISK_AUDIT_USAGE_LINE}");
                return Ok(());
            }
            _ => return Err(ExitError::usage(usage())),
        }
        index += 1;
    }

    let span = tracing::Span::current();
    span.record("root", root.to_string_lossy().to_string().as_str());
    span.record(
        "rules",
        rules_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    span.record("json", json);

    let envelope = run_risk_audit(&root, rules_path.as_deref());

    // F11.4: best-effort TraceEvent emission for the standalone CLI. The
    // trace log lives under `<root>/.forge-method`; when the root is not a
    // Forge project (no `.forge-method` dir), skip persistence so the command
    // never pollutes an unrelated tree. This keeps the standalone usable as
    // a generic auditor while still leaving a trail inside Forge projects.
    let trace_state_root = root.join(".forge-method");
    if trace_state_root.is_dir() {
        let now_unix = resolve_now_unix(None);
        let trace_id = format!("risk-audit-standalone-{now_unix}");
        let recorded_at = format!("{now_unix}");
        let rule_set_ref = rules_path
            .as_deref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let (error_count, warning_count, target_count) =
            envelope.data.as_ref().map_or((0, 0, 0), |summary| {
                (
                    summary.error_count,
                    summary.warning_count,
                    summary.target_count,
                )
            });
        let structural_error = if envelope.data.is_none() {
            envelope
                .error
                .as_ref()
                .map(|e| e.message.clone())
                .or_else(|| Some("risk-audit could not produce a summary".to_string()))
        } else {
            None
        };
        let ctx = crate::risk_audit_trace::RiskAuditTraceContext {
            trace_id: &trace_id,
            run_id: &trace_id,
            recorded_at: &recorded_at,
            principal_id: "forge-core",
            agent_id: "risk-audit",
            rule_set_ref: &rule_set_ref,
        };
        let events = crate::risk_audit_trace::build_risk_audit_events(
            &ctx,
            error_count,
            warning_count,
            target_count,
            structural_error.as_deref(),
        );
        for event in &events {
            if let Err(source) = append_trace_event(&trace_state_root, event) {
                eprintln!("forge-core: risk-audit trace append failed (non-fatal): {source}");
            }
        }
    }

    span.record(
        "target_count",
        envelope
            .data
            .as_ref()
            .map_or(0, |d: &RiskAuditSummary| d.target_count),
    );
    span.record(
        "diagnostic_count",
        envelope
            .data
            .as_ref()
            .map_or(0, |d: &RiskAuditSummary| d.diagnostic_count),
    );

    let passed = envelope.ok;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&envelope).expect("serialize risk-audit envelope")
        );
    } else {
        print_human(&envelope);
    }

    if passed {
        Ok(())
    } else {
        // The envelope's `exit_code()` is the source of truth for the shell
        // exit; route it through `ExitError::with_code` with the diagnostic.
        let code = envelope.exit_code();
        let message = envelope
            .error
            .as_ref()
            .map(|e| e.message.clone())
            .unwrap_or_default();
        Err(ExitError::with_code(code, message))
    }
}

fn print_human(envelope: &CliEnvelope<RiskAuditSummary>) {
    match (&envelope.data, &envelope.error) {
        (Some(summary), _) => {
            println!(
                "risk-audit: {} rules, {} targets, {} diagnostics ({} errors, {} warnings)",
                summary.rule_count,
                summary.target_count,
                summary.diagnostic_count,
                summary.error_count,
                summary.warning_count,
            );
            for diag in &summary.diagnostics {
                eprintln!(
                    "{} {} {}: {}",
                    diag.severity, diag.code, diag.path, diag.message
                );
            }
            if summary.passed() {
                println!("risk-audit: passed (no errors)");
            } else {
                println!("risk-audit: FAILED ({} errors)", summary.error_count);
            }
        }
        (None, Some(err)) => {
            eprintln!("risk-audit: {}", err.message);
        }
        (None, None) => {
            eprintln!("risk-audit: failed without diagnostic");
        }
    }
}

fn run_risk_audit(root: &Path, rules_path: Option<&Path>) -> CliEnvelope<RiskAuditSummary> {
    let command = "risk-audit.run";

    let Some(rules_path) = rules_path else {
        return CliEnvelope::err(
            command,
            RiskAuditError::MissingRules.exit_reason(),
            RiskAuditError::MissingRules.to_string(),
        );
    };

    let rules_yaml = match fs::read_to_string(rules_path) {
        Ok(content) => content,
        Err(source) => {
            return CliEnvelope::err(
                command,
                RiskAuditError::RulesUnreadable {
                    path: rules_path.to_string_lossy().to_string(),
                    source: source.to_string(),
                }
                .exit_reason(),
                RiskAuditError::RulesUnreadable {
                    path: rules_path.to_string_lossy().to_string(),
                    source: source.to_string(),
                }
                .to_string(),
            );
        }
    };

    let ruleset: RiskAuditRuleSet = match yaml_serde::from_str(&rules_yaml) {
        Ok(value) => value,
        Err(source) => {
            return CliEnvelope::err(
                command,
                RiskAuditError::RulesParseFailed {
                    source: source.to_string(),
                }
                .exit_reason(),
                RiskAuditError::RulesParseFailed {
                    source: source.to_string(),
                }
                .to_string(),
            );
        }
    };

    let structure_report = validate_risk_audit_rule_set(&ruleset);
    if structure_report.has_errors() {
        let first_error = structure_report
            .diagnostics()
            .iter()
            .find(|d| d.severity == forge_core_validate::DiagnosticSeverity::Error)
            .map_or_else(
                || "unknown structural error".to_string(),
                |d| format!("{}: {}", d.path, d.message),
            );
        return CliEnvelope::err(
            command,
            RiskAuditError::RuleSetInvalid {
                first_error: first_error.clone(),
            }
            .exit_reason(),
            RiskAuditError::RuleSetInvalid { first_error }.to_string(),
        );
    }

    let targets = match collect_risk_audit_targets(root) {
        Ok(targets) => targets,
        Err(source) => {
            return CliEnvelope::err(
                command,
                ExitReason::EnvConfig,
                format!("could not walk `{}`: {source}", root.display()),
            );
        }
    };

    let findings = evaluate_risk_audit(&ruleset, &targets);

    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    let diagnostics: Vec<RiskAuditDiagnosticView> = findings
        .diagnostics()
        .iter()
        .map(|d| {
            let severity = match d.severity {
                forge_core_validate::DiagnosticSeverity::Error => {
                    error_count += 1;
                    "error"
                }
                forge_core_validate::DiagnosticSeverity::Warning => {
                    warning_count += 1;
                    "warning"
                }
            };
            RiskAuditDiagnosticView {
                severity: severity.to_string(),
                code: format!("{:?}", d.code),
                path: d.path.clone(),
                message: d.message.clone(),
            }
        })
        .collect();

    let summary = RiskAuditSummary {
        rule_count: ruleset.rules.len(),
        target_count: targets.len(),
        diagnostic_count: diagnostics.len(),
        error_count,
        warning_count,
        diagnostics,
    };

    if summary.passed() {
        CliEnvelope::ok(command, summary)
    } else {
        // Fail closed but still surface the full diagnostic payload so agents
        // can act on every finding without re-running.
        CliEnvelope::reject(
            command,
            ExitReason::RejectedByGate,
            format!("risk-audit failed with {} errors", summary.error_count),
            summary,
        )
    }
}
