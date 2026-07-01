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
}
