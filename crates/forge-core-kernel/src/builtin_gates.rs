//! V3.A — the two built-in mutation gates: risk-audit and citation.
//!
//! These are the concrete [`OperationGate`] implementations that the kernel
//! runs internally, before any WAL append. The CLI/risk-audit crates no longer
//! run the checks inline; instead they construct the gates from config (the
//! `--require-risk-audit` / `--require-citation` flags) and attach them to the
//! [`crate::RuntimeOperationExecutionContext`] via `.with_gate(Box::new(...))`.
//!
//! Each gate:
//! - holds only config + the pre-resolved data it needs (the rule set, the
//!   curated evidence registry, the runtime source ids, the trace identity),
//! - calls the *unmodified* evaluator in `forge-core-validate`
//!   (`evaluate_risk_audit`, `validate_yaml_citation_references`),
//! - owns its own telemetry: the risk-audit gate emits `TraceEvent`s so
//!   `forge explain` can narrate the audit.
//!
//! The gates stay fail-closed and synchronous (ADR-0001): `evaluate` returns
//! `Err(GateRejection)` to block the WAL append.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use forge_core_contracts::FieldEvidenceRegistry;
use forge_core_store::{append_trace_event, collect_validation_yaml_documents};
use forge_core_trace::{
    TraceActor, TraceAuthority, TraceCost, TraceEvent, TraceEventKind, TraceRef, TraceRisk,
    TraceRiskLevel,
};
use forge_core_validate::risk_audit::{
    collect_risk_audit_targets, evaluate_risk_audit, validate_risk_audit_rule_set, RiskAuditRuleSet,
};
use forge_core_validate::{validate_yaml_citation_references, DiagnosticSeverity};

use crate::gate::{GateRejection, OperationGate};
use crate::planning::RuntimePlan;

// ---------------------------------------------------------------------------
// Risk-audit gate
// ---------------------------------------------------------------------------

/// The risk-audit mutation gate. Runs the configured `risk-audit-v0` rule set
/// against the project tree and fails closed on any `Error`-severity finding.
///
/// The gate owns the pre-loaded rule set and the trace identity used to emit
/// `TraceEvent`s. It calls [`evaluate_risk_audit`] unchanged and counts the
/// `Error`-severity diagnostics. Trace persistence is best-effort: a trace
/// write failure is logged to stderr and never overrides the gate decision.
pub struct RiskAuditGate {
    /// Pre-parsed, structurally-validated rule set.
    pub ruleset: RiskAuditRuleSet,
    /// Repo root to walk for audit targets.
    pub project_root: PathBuf,
    /// State root (the `.forge-method` parent) where trace events persist.
    pub trace_state_root: PathBuf,
    /// Trace identity. `run_id` / `recorded_at` distinguish one audit pass.
    pub trace_id: String,
    pub run_id: String,
    pub recorded_at: String,
    /// Repo-relative reference to the rule set file, for the trace event.
    pub rule_set_ref: String,
}

impl OperationGate for RiskAuditGate {
    fn evaluate(&self, _plan: &RuntimePlan) -> Result<(), GateRejection> {
        // 1) Structural validation of the rule set. If the rule set is
        //    malformed, emit trace + reject without walking the tree.
        let structure_report = validate_risk_audit_rule_set(&self.ruleset);
        if structure_report.has_errors() {
            let first_error = structure_report
                .diagnostics()
                .iter()
                .find(|d| d.severity == DiagnosticSeverity::Error)
                .map_or_else(
                    || "unknown structural error".to_string(),
                    |d| format!("{}: {}", d.path, d.message),
                );
            let count = structure_report.diagnostics().len();
            self.emit_trace(count, 0, 0, Some(&first_error));
            return Err(structural_rejection(&first_error, count));
        }
        // 2) Walk the tree and run the evaluator.
        let targets = match collect_risk_audit_targets(&self.project_root) {
            Ok(targets) => targets,
            Err(source) => {
                // The walk itself failed — treat as a structural/config
                // rejection so the mutation is blocked. Emit a trace.
                let message = format!("risk-audit collect_targets: {source}");
                self.emit_trace(1, 0, 0, Some(&message));
                return Err(GateRejection::RiskAuditFailed {
                    error_count: 1,
                    finding_paths: vec![message],
                });
            }
        };
        let target_count = targets.len();
        let findings = evaluate_risk_audit(&self.ruleset, &targets);
        let error_count = findings.error_count();
        let warning_count = findings.warning_count();
        self.emit_trace(error_count, warning_count, target_count, None);
        if findings.has_errors() {
            let finding_paths: Vec<String> = findings
                .diagnostics()
                .iter()
                .filter(|d| d.severity == DiagnosticSeverity::Error)
                .map(|d| format!("{}: {}", d.path, d.message))
                .collect();
            Err(GateRejection::RiskAuditFailed {
                error_count,
                finding_paths,
            })
        } else {
            Ok(())
        }
    }

    fn name(&self) -> &'static str {
        "risk-audit"
    }
}

impl RiskAuditGate {
    /// Best-effort trace emission. Persistence failures are logged to stderr
    /// and never change the gate outcome.
    fn emit_trace(
        &self,
        error_count: usize,
        warning_count: usize,
        target_count: usize,
        structural_error: Option<&str>,
    ) {
        let ctx = RiskAuditTraceContext {
            trace_id: &self.trace_id,
            run_id: &self.run_id,
            recorded_at: &self.recorded_at,
            rule_set_ref: &self.rule_set_ref,
        };
        let events = build_risk_audit_events(
            &ctx,
            error_count,
            warning_count,
            target_count,
            structural_error,
        );
        let _ = fs::create_dir_all(&self.trace_state_root);
        for event in &events {
            if let Err(source) = append_trace_event(&self.trace_state_root, event) {
                eprintln!("forge-core: risk-audit trace append failed (non-fatal): {source}");
            }
        }
    }
}

/// Build a `RiskAuditFailed` rejection for a structurally-invalid rule set.
/// Free function (does not read `&self`) — the gate calls it after emitting
/// its trace.
fn structural_rejection(first_error: &str, count: usize) -> GateRejection {
    GateRejection::RiskAuditFailed {
        error_count: count,
        finding_paths: vec![first_error.to_string()],
    }
}

// ---------------------------------------------------------------------------
// Citation gate
// ---------------------------------------------------------------------------

/// The F14 citation mutation gate. Resolves every `source_id` in the workspace
/// YAML against the joint curated [`FieldEvidenceRegistry`] ∪ the runtime
/// Source Ledger ids, failing closed on any unresolved id.
///
/// The gate takes the *data* it needs (the curated registry and the runtime
/// id set) rather than a `ResearchProjection`, so the kernel stays decoupled
/// from `forge-core-research` (which depends back through store/validate).
/// The CLI resolves the runtime half and passes it in.
pub struct CitationGate {
    /// Repo root to walk for YAML documents.
    pub project_root: PathBuf,
    /// Curated field-evidence registry (best-effort: empty when absent).
    pub evidence: FieldEvidenceRegistry,
    /// Live runtime Source Ledger ids (keys of `ResearchProjection::sources`).
    pub runtime_ids: HashSet<String>,
}

impl OperationGate for CitationGate {
    fn evaluate(&self, _plan: &RuntimePlan) -> Result<(), GateRejection> {
        let documents = collect_validation_yaml_documents(&self.project_root);
        let report = validate_yaml_citation_references(
            &documents.documents,
            &self.evidence,
            &self.runtime_ids,
        );
        if report.has_errors() {
            let unresolved: Vec<String> = report
                .diagnostics()
                .iter()
                .filter(|d| d.severity == DiagnosticSeverity::Error)
                .map(|d| format!("{}: {}", d.path, d.message))
                .collect();
            Err(GateRejection::CitationCheckFailed {
                unresolved_source_ids: unresolved,
            })
        } else {
            Ok(())
        }
    }

    fn name(&self) -> &'static str {
        "citation"
    }
}

// ---------------------------------------------------------------------------
// Risk-audit trace event construction (moved from the CLI).
// ---------------------------------------------------------------------------

/// Shared identity/addressing fields for every risk-audit trace event. The
/// gate always emits events against the same run/actor/rule-set, so bundling
/// them clarifies the call sites. `principal_id` / `agent_id` are fixed for
/// the kernel-emitted gate (`forge-core` / `execute-operation`).
struct RiskAuditTraceContext<'a> {
    trace_id: &'a str,
    run_id: &'a str,
    recorded_at: &'a str,
    rule_set_ref: &'a str,
}

/// Build the trace events for one risk-audit pass. Emits `RiskAuditStarted`
/// unconditionally, then `RiskAuditPassed` or `RiskAuditFailed` depending on
/// `error_count`. When the rule set is structurally invalid, the failed event
/// carries that context in its message instead of a finding count.
#[must_use]
fn build_risk_audit_events(
    ctx: &RiskAuditTraceContext<'_>,
    error_count: usize,
    warning_count: usize,
    target_count: usize,
    structural_error: Option<&str>,
) -> Vec<TraceEvent> {
    let started = risk_audit_event(
        ctx,
        "started",
        TraceEventKind::RiskAuditStarted,
        format!(
            "risk-audit started: {rule_count} rule(s) against {target_count} target(s)",
            rule_count = "loaded",
            target_count = target_count,
        ),
    );
    let outcome_kind = if error_count == 0 {
        TraceEventKind::RiskAuditPassed
    } else {
        TraceEventKind::RiskAuditFailed
    };
    let outcome_message = if let Some(error) = structural_error {
        format!("risk-audit failed: rule set invalid: {error}")
    } else if error_count == 0 {
        format!(
            "risk-audit passed: 0 error(s), {warning_count} warning(s) across {target_count} target(s)"
        )
    } else {
        format!(
            "risk-audit failed: {error_count} error(s), {warning_count} warning(s) across {target_count} target(s)"
        )
    };
    let outcome = risk_audit_event(ctx, "outcome", outcome_kind, outcome_message);
    vec![started, outcome]
}

fn risk_audit_event(
    ctx: &RiskAuditTraceContext<'_>,
    suffix: &str,
    kind: TraceEventKind,
    message: String,
) -> TraceEvent {
    let event_id = format!("{}.risk-audit.{}", ctx.run_id, suffix);
    // Only `RiskAuditFailed` raises the risk level; every other variant stays Low.
    let risk_level = if matches!(kind, TraceEventKind::RiskAuditFailed) {
        TraceRiskLevel::Blocked
    } else {
        TraceRiskLevel::Low
    };
    TraceEvent::new(
        ctx.trace_id,
        ctx.run_id,
        event_id,
        kind,
        ctx.recorded_at,
        message,
    )
    .with_actor(TraceActor::new(
        "forge-core",
        "execute-operation",
        "auditor",
    ))
    .with_authority(TraceAuthority::for_operation("risk-audit"))
    .with_risk(TraceRisk::new(risk_level, false))
    .with_cost(TraceCost::zero())
    .with_inputs(vec![TraceRef::new("risk_audit_rules", ctx.rule_set_ref)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_validate::risk_audit::{
        RiskAuditRule, RiskAuditRuleSet, RISK_AUDIT_SCHEMA_VERSION,
    };

    fn empty_ruleset() -> RiskAuditRuleSet {
        RiskAuditRuleSet {
            schema_version: RISK_AUDIT_SCHEMA_VERSION.to_string(),
            rules: vec![],
        }
    }

    fn rule(id: &str, pattern: &str) -> RiskAuditRule {
        RiskAuditRule {
            id: id.to_string(),
            description: "test rule".to_string(),
            severity: forge_core_validate::risk_audit::RiskAuditSeverity::Error,
            detector: forge_core_validate::risk_audit::RiskAuditDetector::Regex {
                pattern: pattern.to_string(),
            },
            evidence_required: false,
            fix_hint: "fix".to_string(),
            applies_to: vec!["**/*.rs".to_string()],
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-core-kernel-gate-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp root");
        path
    }

    #[test]
    fn risk_audit_gate_passes_clean_tree() {
        let root = temp_dir("clean");
        let mut ruleset = empty_ruleset();
        ruleset.rules.push(rule("no-unwrap", r"\.unwrap\(\)"));
        let gate = RiskAuditGate {
            ruleset,
            project_root: root.clone(),
            trace_state_root: root.join(".forge-method"),
            trace_id: "trace.1".to_string(),
            run_id: "run.1".to_string(),
            recorded_at: "2026-07-02T00:00:00Z".to_string(),
            rule_set_ref: "rules.yaml".to_string(),
        };
        // A stub plan is enough — the gate ignores the plan body.
        let plan = stub_plan();
        assert!(gate.evaluate(&plan).is_ok());
    }

    #[test]
    fn risk_audit_gate_fails_on_error_finding() {
        let root = temp_dir("dirty");
        fs::create_dir_all(root.join("src")).expect("create src dir");
        fs::write(root.join("src").join("lib.rs"), "let x = opt.unwrap();\n").expect("write file");
        let mut ruleset = empty_ruleset();
        ruleset.rules.push(rule("no-unwrap", r"\.unwrap\(\)"));
        let gate = RiskAuditGate {
            ruleset,
            project_root: root,
            trace_state_root: std::env::temp_dir(),
            trace_id: "trace.2".to_string(),
            run_id: "run.2".to_string(),
            recorded_at: "2026-07-02T00:00:00Z".to_string(),
            rule_set_ref: "rules.yaml".to_string(),
        };
        let plan = stub_plan();
        let rejection = gate.evaluate(&plan).expect_err("should reject");
        match rejection {
            GateRejection::RiskAuditFailed {
                error_count,
                finding_paths,
            } => {
                assert!(error_count >= 1);
                assert!(!finding_paths.is_empty());
            }
            other => panic!("expected RiskAuditFailed, got {other:?}"),
        }
    }

    #[test]
    fn risk_audit_gate_rejects_structurally_invalid_ruleset() {
        let root = temp_dir("bad-ruleset");
        // Empty rule set is structurally invalid.
        let gate = RiskAuditGate {
            ruleset: empty_ruleset(),
            project_root: root,
            trace_state_root: std::env::temp_dir(),
            trace_id: "trace.3".to_string(),
            run_id: "run.3".to_string(),
            recorded_at: "2026-07-02T00:00:00Z".to_string(),
            rule_set_ref: "rules.yaml".to_string(),
        };
        let plan = stub_plan();
        assert!(matches!(
            gate.evaluate(&plan),
            Err(GateRejection::RiskAuditFailed { .. })
        ));
    }

    #[test]
    fn citation_gate_passes_when_no_source_ids() {
        let root = temp_dir("citation-clean");
        let gate = CitationGate {
            project_root: root,
            evidence: empty_evidence_registry(),
            runtime_ids: HashSet::new(),
        };
        let plan = stub_plan();
        assert!(gate.evaluate(&plan).is_ok());
    }

    /// Minimal empty `FieldEvidenceRegistry` for tests (the struct does not
    /// derive `Default`). Mirrors the CLI's `empty_evidence` fallback.
    fn empty_evidence_registry() -> FieldEvidenceRegistry {
        use forge_core_contracts::evidence::{EvidencePolicy, GeographicCoverage};
        FieldEvidenceRegistry {
            schema_version: String::new(),
            research: String::new(),
            created_at: String::new(),
            status: String::new(),
            policy: EvidencePolicy {
                purpose: String::new(),
                evidence_tiers: Vec::new(),
                rule: String::new(),
                geographic_coverage: GeographicCoverage {
                    rule: String::new(),
                    rationale: String::new(),
                    minimum_behavior: Vec::new(),
                },
            },
            sources: Vec::new(),
            plan_level_implications: Vec::new(),
            open_research_gaps: Vec::new(),
        }
    }

    /// Minimal `RuntimePlan` — the built-in gates never read the plan body, so
    /// a default-constructed plan is enough to drive `evaluate`.
    fn stub_plan() -> RuntimePlan {
        use forge_core_contracts::operation::{
            AutonomyMode, ExecutionMode, HumanInputRequirement, MutationPolicy, NextActor,
            OperationGateStatus, OperationSideEffectPolicy,
        };
        use forge_core_contracts::StableId;
        RuntimePlan {
            status: crate::planning::RuntimePlanStatus::ReadyToCallOperation,
            contract_id: StableId("stub".to_string()),
            autonomy_mode: AutonomyMode::Observe,
            next_actor: NextActor::ForgeCore,
            host_action: forge_core_contracts::operation::HostAction::ShowStatus,
            next_operation: None,
            phase: StableId("p".to_string()),
            workflow: StableId("w".to_string()),
            action: StableId("a".to_string()),
            mutation_policy: MutationPolicy::Forbidden,
            side_effect_policy: OperationSideEffectPolicy::WriteProjectFiles,
            execution_mode: ExecutionMode::ObserveOnly,
            gate_status: OperationGateStatus::Pass,
            human_input_requirement: HumanInputRequirement::None,
            prompt: None,
            command_refs: Vec::new(),
            effect_contract_refs: Vec::new(),
            reasons: Vec::new(),
            validation_error_count: 0,
            validation_warning_count: 0,
            reference_error_count: 0,
            reference_warning_count: 0,
            used_read_snapshot: false,
        }
    }
}
