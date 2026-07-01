use serde::{Deserialize, Serialize};

pub const TRACE_EVENT_SCHEMA_VERSION: &str = "0.1";
pub const TRACE_EVENT_KIND: &str = "trace_event";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEvent {
    pub schema_version: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub trace_id: String,
    pub event_id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub event_kind: TraceEventKind,
    pub recorded_at: String,
    pub actor: TraceActor,
    pub authority: TraceAuthority,
    pub inputs: Vec<TraceRef>,
    pub outputs: Vec<TraceRef>,
    pub risk: TraceRisk,
    pub cost: TraceCost,
    pub message: String,
}

impl TraceEvent {
    #[must_use]
    pub fn new(
        trace_id: impl Into<String>,
        run_id: impl Into<String>,
        event_id: impl Into<String>,
        event_kind: TraceEventKind,
        recorded_at: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: TRACE_EVENT_SCHEMA_VERSION.to_string(),
            kind: TRACE_EVENT_KIND.to_string(),
            project_id: None,
            trace_id: trace_id.into(),
            event_id: event_id.into(),
            run_id: run_id.into(),
            graph_id: None,
            node_id: None,
            event_kind,
            recorded_at: recorded_at.into(),
            actor: TraceActor::unknown(),
            authority: TraceAuthority::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            risk: TraceRisk::unknown(),
            cost: TraceCost::zero(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    #[must_use]
    pub fn with_actor(mut self, actor: TraceActor) -> Self {
        self.actor = actor;
        self
    }

    #[must_use]
    pub fn with_authority(mut self, authority: TraceAuthority) -> Self {
        self.authority = authority;
        self
    }

    #[must_use]
    pub fn with_risk(mut self, risk: TraceRisk) -> Self {
        self.risk = risk;
        self
    }

    #[must_use]
    pub fn with_cost(mut self, cost: TraceCost) -> Self {
        self.cost = cost;
        self
    }

    #[must_use]
    pub fn with_inputs(mut self, inputs: Vec<TraceRef>) -> Self {
        self.inputs = inputs;
        self
    }

    #[must_use]
    pub fn with_outputs(mut self, outputs: Vec<TraceRef>) -> Self {
        self.outputs = outputs;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    RunStarted,
    OperationPlanned,
    PreviewCompleted,
    ReadyCompleted,
    GatePassed,
    GateBlocked,
    EffectStaged,
    EffectApplied,
    RunCompleted,
    RunFailed,
    /// F11.4: risk-audit pass started (rule set loaded, walk beginning).
    /// Emitted by both the standalone `risk-audit` CLI and the
    /// `execute-operation --require-risk-audit` gate.
    RiskAuditStarted,
    /// F11.4: risk-audit completed with zero Error-severity findings.
    RiskAuditPassed,
    /// F11.4: risk-audit failed closed (structural errors or findings).
    RiskAuditFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceActor {
    pub principal_id: String,
    pub agent_id: String,
    pub role: String,
}

impl TraceActor {
    #[must_use]
    pub fn new(
        principal_id: impl Into<String>,
        agent_id: impl Into<String>,
        role: impl Into<String>,
    ) -> Self {
        Self {
            principal_id: principal_id.into(),
            agent_id: agent_id.into(),
            role: role.into(),
        }
    }

    #[must_use]
    pub fn unknown() -> Self {
        Self::new("principal.unknown", "agent.unknown", "unknown")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceAuthority {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub capability_ids: Vec<String>,
}

impl TraceAuthority {
    #[must_use]
    pub fn for_operation(operation_id: impl Into<String>) -> Self {
        Self {
            operation_id: Some(operation_id.into()),
            capability_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceRef {
    pub ref_kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

impl TraceRef {
    #[must_use]
    pub fn new(ref_kind: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            ref_kind: ref_kind.into(),
            reference: reference.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceRisk {
    pub risk_level: TraceRiskLevel,
    pub destructive: bool,
}

impl TraceRisk {
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            risk_level: TraceRiskLevel::Unknown,
            destructive: false,
        }
    }

    #[must_use]
    pub const fn new(risk_level: TraceRiskLevel, destructive: bool) -> Self {
        Self {
            risk_level,
            destructive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceRiskLevel {
    Unknown,
    Low,
    Medium,
    High,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceCost {
    pub model_calls: u64,
    pub tool_calls: u64,
    pub estimated_tokens: u64,
}

impl TraceCost {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            model_calls: 0,
            tool_calls: 0,
            estimated_tokens: 0,
        }
    }
}

// ===========================================================================
// F13: Budget/Cost Accounting.
//
// Pure aggregation over a slice of TraceEvents. The CLI `forge-core cost`
// command loads events via query_trace_events and hands them here. Keeping
// aggregation pure + in this crate means it is unit-testable without any
// filesystem or CLI harness.
// ===========================================================================

/// Scope that a [`CostReport`] was built for. Mirrors the CLI filter that
/// selected the events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostScope {
    Run,
    Graph,
    Principal,
    All,
}

/// Aggregated cost totals. `event_count` is the number of trace events that
/// contributed to the other fields, so a caller can tell an empty report from
/// a report over zero-cost events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CostTotals {
    pub model_calls: u64,
    pub tool_calls: u64,
    pub estimated_tokens: u64,
    pub event_count: u64,
}

impl CostTotals {
    /// Accumulate another set of totals into this one.
    pub fn add(&mut self, other: &CostTotals) {
        self.model_calls += other.model_calls;
        self.tool_calls += other.tool_calls;
        self.estimated_tokens += other.estimated_tokens;
        self.event_count += other.event_count;
    }
}

/// One row in a breakdown (per-run or per-agent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostBreakdownEntry {
    pub key: String,
    pub totals: CostTotals,
}

/// Aggregated cost report returned by [`aggregate_costs`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostReport {
    pub schema_version: String,
    pub scope: CostScope,
    pub scope_id: String,
    pub totals: CostTotals,
    pub by_run: Vec<CostBreakdownEntry>,
    pub by_agent: Vec<CostBreakdownEntry>,
}

impl CostReport {
    /// Schema version tag carried on every report.
    pub const SCHEMA_VERSION: &'static str = "cost-report-v0";
}

/// Aggregate cost over a slice of trace events.
///
/// `scope` and `scope_id` describe the filter the caller applied (they are
/// not re-derived here); the function always aggregates every event it is
/// given. Breakdowns are sorted by descending `estimated_tokens` so the
/// heaviest contributors surface first.
#[must_use]
pub fn aggregate_costs(events: &[TraceEvent], scope: CostScope, scope_id: &str) -> CostReport {
    let mut totals = CostTotals::default();
    let mut by_run: std::collections::BTreeMap<String, CostTotals> =
        std::collections::BTreeMap::new();
    let mut by_agent: std::collections::BTreeMap<String, CostTotals> =
        std::collections::BTreeMap::new();
    for event in events {
        let row = CostTotals {
            model_calls: event.cost.model_calls,
            tool_calls: event.cost.tool_calls,
            estimated_tokens: event.cost.estimated_tokens,
            event_count: 1,
        };
        totals.add(&row);
        by_run.entry(event.run_id.clone()).or_default().add(&row);
        by_agent
            .entry(event.actor.agent_id.clone())
            .or_default()
            .add(&row);
    }
    let sort_desc =
        |map: std::collections::BTreeMap<String, CostTotals>| -> Vec<CostBreakdownEntry> {
            let mut entries: Vec<CostBreakdownEntry> = map
                .into_iter()
                .map(|(key, totals)| CostBreakdownEntry { key, totals })
                .collect();
            entries.sort_by(|a, b| b.totals.estimated_tokens.cmp(&a.totals.estimated_tokens));
            entries
        };
    CostReport {
        schema_version: CostReport::SCHEMA_VERSION.to_string(),
        scope,
        scope_id: scope_id.to_string(),
        totals,
        by_run: sort_desc(by_run),
        by_agent: sort_desc(by_agent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_event_round_trips_canonical_shape() {
        let event = TraceEvent::new(
            "trace.example",
            "run.example",
            "evt.0001",
            TraceEventKind::RunStarted,
            "2026-06-28T00:00:00Z",
            "Run started",
        )
        .with_project_id("forge-method-core")
        .with_actor(TraceActor::new(
            "principal.human.daniel",
            "agent.codex.local",
            "driver",
        ))
        .with_authority(TraceAuthority::for_operation("op.example"))
        .with_inputs(vec![TraceRef::new("operation", "docs/fixtures/op.yaml")])
        .with_risk(TraceRisk::new(TraceRiskLevel::Low, false));

        let json = serde_json::to_string(&event).expect("serialize trace event");
        assert!(json.contains("\"kind\":\"trace_event\""));
        assert!(json.contains("\"event_kind\":\"run_started\""));
        assert!(json.contains("\"project_id\":\"forge-method-core\""));

        let decoded: TraceEvent = serde_json::from_str(&json).expect("decode trace event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn optional_context_is_omitted_when_absent() {
        let event = TraceEvent::new(
            "trace.example",
            "run.example",
            "evt.0001",
            TraceEventKind::ReadyCompleted,
            "unknown",
            "Ready completed",
        );
        let json = serde_json::to_string(&event).expect("serialize trace event");

        assert!(!json.contains("project_id"));
        assert!(!json.contains("graph_id"));
        assert!(!json.contains("node_id"));
    }

    fn cost_event(
        run_id: &str,
        agent_id: &str,
        model_calls: u64,
        tool_calls: u64,
        tokens: u64,
    ) -> TraceEvent {
        TraceEvent::new(
            "trace.cost",
            run_id,
            format!("{run_id}.evt"),
            TraceEventKind::EffectApplied,
            "2026-06-30T00:00:00Z",
            "effect applied",
        )
        .with_actor(TraceActor::new("principal", agent_id, "driver"))
        .with_cost(TraceCost {
            model_calls,
            tool_calls,
            estimated_tokens: tokens,
        })
    }

    #[test]
    fn aggregate_costs_sums_totals_across_events() {
        let events = vec![
            cost_event("run.a", "agent-1", 10, 5, 1_000),
            cost_event("run.a", "agent-2", 3, 2, 500),
            cost_event("run.b", "agent-1", 7, 1, 2_000),
        ];
        let report = aggregate_costs(&events, CostScope::All, "*");
        assert_eq!(report.totals.model_calls, 20);
        assert_eq!(report.totals.tool_calls, 8);
        assert_eq!(report.totals.estimated_tokens, 3_500);
        assert_eq!(report.totals.event_count, 3);
    }

    #[test]
    fn aggregate_costs_breaks_down_by_run_and_agent() {
        let events = vec![
            cost_event("run.a", "agent-1", 10, 0, 1_000),
            cost_event("run.b", "agent-1", 0, 0, 5_000),
            cost_event("run.a", "agent-2", 0, 0, 500),
        ];
        let report = aggregate_costs(&events, CostScope::All, "*");
        // by_run sorted by descending tokens: run.b (5_000), run.a (1_500).
        assert_eq!(report.by_run.len(), 2);
        assert_eq!(report.by_run[0].key, "run.b");
        assert_eq!(report.by_run[0].totals.estimated_tokens, 5_000);
        assert_eq!(report.by_run[1].key, "run.a");
        // by_agent sorted by descending tokens: agent-1 (6_000), agent-2 (500).
        assert_eq!(report.by_agent.len(), 2);
        assert_eq!(report.by_agent[0].key, "agent-1");
        assert_eq!(report.by_agent[1].key, "agent-2");
    }

    #[test]
    fn aggregate_costs_empty_slice_is_zero_not_missing() {
        let report = aggregate_costs(&[], CostScope::Run, "run.empty");
        assert_eq!(report.totals, CostTotals::default());
        assert_eq!(report.totals.event_count, 0);
        assert!(report.by_run.is_empty());
        assert!(report.by_agent.is_empty());
        assert_eq!(report.scope, CostScope::Run);
        assert_eq!(report.scope_id, "run.empty");
        assert_eq!(report.schema_version, CostReport::SCHEMA_VERSION);
    }
}
