//! F05.6: `TraceEvent` construction for the eval-harness surface.
//!
//! Pure helper that builds `TraceEvent`s for the three eval-compare lifecycle
//! states (started/passed/failed). The CLI driver (`eval_harness_cmd`) owns
//! state-root resolution and best-effort persistence; this module only shapes
//! the events, so the wire format of an eval-compare event is defined once
//! (deletion test: if this module goes away, the caller re-derives the shape).

use forge_core_eval::{EvalArmLabel, EvalCompareStatus};
use forge_core_trace::{
    TraceActor, TraceAuthority, TraceCost, TraceEvent, TraceEventKind, TraceRef, TraceRisk,
    TraceRiskLevel,
};

/// Shared identity and addressing fields for every eval-harness trace event.
#[derive(Clone, Debug)]
pub struct EvalHarnessTraceContext<'a> {
    pub trace_id: &'a str,
    pub run_id: &'a str,
    pub recorded_at: &'a str,
    pub principal_id: &'a str,
    pub agent_id: &'a str,
    pub config_ref: &'a str,
}

/// Outcome fields the trace builder needs. Bundled into a struct to avoid a
/// nine-argument call site (AGENTS.md pitfall #4: parameter struct over
/// `#[allow]`).
#[derive(Clone, Debug)]
pub struct EvalHarnessOutcome {
    pub status: EvalCompareStatus,
    pub baseline: EvalArmLabel,
    pub candidate: EvalArmLabel,
    pub task_count: usize,
    pub baseline_success_bps: u32,
    pub candidate_success_bps: u32,
    pub recommendation: String,
    pub diagnostic_count: usize,
}

/// Builds the trace events for one eval-harness comparison.
///
/// Emits `EvalCompareStarted` unconditionally, then `EvalComparePassed` or
/// `EvalCompareFailed` depending on the report status. Blocked comparisons
/// raise the risk level to Medium (not Blocked -- eval is advisory, not a
/// mutation gate like risk-audit).
#[must_use]
pub fn build_eval_harness_events(
    ctx: &EvalHarnessTraceContext<'_>,
    outcome: &EvalHarnessOutcome,
) -> Vec<TraceEvent> {
    let started = eval_harness_event(
        ctx,
        "started",
        TraceEventKind::EvalCompareStarted,
        format!(
            "eval-harness started: baseline={baseline} candidate={candidate} tasks={task_count}",
            baseline = outcome.baseline,
            candidate = outcome.candidate,
            task_count = outcome.task_count
        ),
        TraceRiskLevel::Low,
    );
    let outcome_kind = if outcome.status == EvalCompareStatus::Passed {
        TraceEventKind::EvalComparePassed
    } else {
        TraceEventKind::EvalCompareFailed
    };
    let risk_level = if outcome.status == EvalCompareStatus::Passed {
        TraceRiskLevel::Low
    } else {
        TraceRiskLevel::Medium
    };
    let outcome_message = if outcome.status == EvalCompareStatus::Passed {
        format!(
            "eval-harness passed: baseline success_bps={} candidate success_bps={} recommendation={}",
            outcome.baseline_success_bps,
            outcome.candidate_success_bps,
            outcome.recommendation
        )
    } else {
        format!(
            "eval-harness blocked: {} diagnostic(s); recommendation={}",
            outcome.diagnostic_count, outcome.recommendation
        )
    };
    let outcome = eval_harness_event(ctx, "outcome", outcome_kind, outcome_message, risk_level);
    vec![started, outcome]
}

#[allow(clippy::too_many_arguments)]
fn eval_harness_event(
    ctx: &EvalHarnessTraceContext<'_>,
    suffix: &str,
    kind: TraceEventKind,
    message: String,
    risk_level: TraceRiskLevel,
) -> TraceEvent {
    let event_id = format!("{}.eval-harness.{}", ctx.run_id, suffix);
    TraceEvent::new(
        ctx.trace_id,
        ctx.run_id,
        event_id,
        kind,
        ctx.recorded_at,
        message,
    )
    .with_actor(TraceActor::new(ctx.principal_id, ctx.agent_id, "eval-harness"))
    .with_authority(TraceAuthority::for_operation("eval-harness"))
    .with_risk(TraceRisk::new(risk_level, false))
    .with_cost(TraceCost::zero())
    .with_inputs(vec![TraceRef::new("eval_harness_config", ctx.config_ref)])
}

#[cfg(test)]
mod tests {
    #![allow(clippy::pedantic)]

    use super::*;

    fn ctx() -> EvalHarnessTraceContext<'static> {
        EvalHarnessTraceContext {
            trace_id: "trace.1",
            run_id: "run.1",
            recorded_at: "2026-07-01T00:00:00Z",
            principal_id: "forge-core",
            agent_id: "eval-harness",
            config_ref: "contracts/eval-harness/valid-router-compare.yaml",
        }
    }

    #[test]
    fn passed_comparison_emits_started_then_passed_at_low_risk() {
        let events = build_eval_harness_events(
            &ctx(),
            &EvalHarnessOutcome {
                status: EvalCompareStatus::Passed,
                baseline: EvalArmLabel::SingleAgent,
                candidate: EvalArmLabel::Mas,
                task_count: 5,
                baseline_success_bps: 10_000,
                candidate_success_bps: 8_000,
                recommendation: "keep_baseline".to_string(),
                diagnostic_count: 0,
            },
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_kind, TraceEventKind::EvalCompareStarted);
        assert_eq!(events[1].event_kind, TraceEventKind::EvalComparePassed);
        assert_eq!(events[1].risk.risk_level, TraceRiskLevel::Low);
        assert!(events[1].message.contains("baseline success_bps=10000"));
    }

    #[test]
    fn blocked_comparison_emits_failed_at_medium_risk() {
        let events = build_eval_harness_events(
            &ctx(),
            &EvalHarnessOutcome {
                status: EvalCompareStatus::Blocked,
                baseline: EvalArmLabel::SingleAgent,
                candidate: EvalArmLabel::Mas,
                task_count: 2,
                baseline_success_bps: 5_000,
                candidate_success_bps: 4_000,
                recommendation: "block_candidate".to_string(),
                diagnostic_count: 3,
            },
        );
        assert_eq!(events[1].event_kind, TraceEventKind::EvalCompareFailed);
        assert_eq!(events[1].risk.risk_level, TraceRiskLevel::Medium);
        assert!(events[1].message.contains("3 diagnostic(s)"));
    }

    #[test]
    fn event_ids_are_unique_within_one_run() {
        let events = build_eval_harness_events(
            &ctx(),
            &EvalHarnessOutcome {
                status: EvalCompareStatus::Passed,
                baseline: EvalArmLabel::SingleAgent,
                candidate: EvalArmLabel::Mas,
                task_count: 1,
                baseline_success_bps: 0,
                candidate_success_bps: 0,
                recommendation: "inconclusive".to_string(),
                diagnostic_count: 0,
            },
        );
        assert_ne!(events[0].event_id, events[1].event_id);
    }
}
