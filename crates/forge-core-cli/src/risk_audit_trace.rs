//! F11.4: `TraceEvent` construction for the risk-audit surface.
//!
//! Pure helpers that build `TraceEvent`s for the three risk-audit lifecycle
//! states (started/passed/failed). Used by two callers:
//!
//! - `run_execute_operation` gate (`--require-risk-audit`, F11.3)
//! - `run_risk_audit` standalone CLI (F11.1)
//!
//! Both callers own their own `state_root` resolution and persistence; this
//! module only shapes the events. Keeping construction here means the wire
//! format of a risk-audit event is defined once (deletion test: if this
//! module goes away, both callers must re-derive the event shape, so it earns
//! its life).

use forge_core_trace::{
    TraceActor, TraceAuthority, TraceCost, TraceEvent, TraceEventKind, TraceRef, TraceRisk,
    TraceRiskLevel,
};

/// Shared identity and addressing fields for every risk-audit trace event.
///
/// The risk-audit surface always emits events against the same run/actor/
/// rule-set, so bundling them clarifies the call sites (depth: one small
/// struct behind which both `build_risk_audit_events` and `risk_audit_event`
/// shape the wire format).
#[derive(Clone, Debug)]
pub struct RiskAuditTraceContext<'a> {
    pub trace_id: &'a str,
    pub run_id: &'a str,
    pub recorded_at: &'a str,
    pub principal_id: &'a str,
    pub agent_id: &'a str,
    pub rule_set_ref: &'a str,
}

/// Build the trace events for one risk-audit pass.
///
/// Emits `RiskAuditStarted` unconditionally, then `RiskAuditPassed` or
/// `RiskAuditFailed` depending on `error_count`. When the rule set itself is
/// structurally invalid (`structural_error` is `Some`), the failed event
/// carries that context in its message instead of a finding count.
#[must_use]
pub fn build_risk_audit_events(
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
            "risk-audit started: {} rule(s) against {} target(s)",
            // rule count is not always known to the caller at started-time;
            // the message is informational, so we phrase it in targets.
            "loaded",
            target_count,
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
    // Only `RiskAuditFailed` raises the risk level; every other variant
    // (started/passed, plus any future addition) stays Low.
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
    .with_actor(TraceActor::new(ctx.principal_id, ctx.agent_id, "auditor"))
    .with_authority(TraceAuthority::for_operation("risk-audit"))
    .with_risk(TraceRisk::new(risk_level, false))
    .with_cost(TraceCost::zero())
    .with_inputs(vec![TraceRef::new("risk_audit_rules", ctx.rule_set_ref)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events_for(error_count: usize, structural_error: Option<&str>) -> Vec<TraceEvent> {
        let ctx = RiskAuditTraceContext {
            trace_id: "trace.1",
            run_id: "run.1",
            recorded_at: "2026-06-30T00:00:00Z",
            principal_id: "principal",
            agent_id: "agent",
            rule_set_ref: "contracts/risk-audits/fail-soft.yaml",
        };
        build_risk_audit_events(&ctx, error_count, 0, 7, structural_error)
    }

    #[test]
    fn passing_audit_emits_started_then_passed() {
        let events = events_for(0, None);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_kind, TraceEventKind::RiskAuditStarted);
        assert_eq!(events[1].event_kind, TraceEventKind::RiskAuditPassed);
        assert!(events[1].message.contains("passed"));
    }

    #[test]
    fn failing_audit_emits_started_then_failed_with_blocked_risk() {
        let events = events_for(3, None);
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].event_kind, TraceEventKind::RiskAuditFailed);
        assert!(events[1].message.contains("3 error(s)"));
        assert_eq!(events[1].risk.risk_level, TraceRiskLevel::Blocked);
    }

    #[test]
    fn structural_error_surfaces_in_failed_message() {
        let events = events_for(1, Some("bad shape"));
        assert_eq!(events[1].event_kind, TraceEventKind::RiskAuditFailed);
        assert!(events[1].message.contains("rule set invalid"));
        assert!(events[1].message.contains("bad shape"));
    }

    #[test]
    fn event_ids_are_unique_within_one_run() {
        let events = events_for(0, None);
        assert_ne!(events[0].event_id, events[1].event_id);
    }
}
