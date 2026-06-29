use crate::common::StableId;
use schemars::JsonSchema;
use serde::de::{Deserializer, Error as DeError};
use serde::{Deserialize, Serialize};

const MAX_SAMPLING_RATE_PER_MYRIAD: u64 = 10_000;

const ALL_TELEMETRY_EVENT_KINDS: [TelemetryEventKind; 9] = [
    TelemetryEventKind::PhaseTransition,
    TelemetryEventKind::ClaimAcquired,
    TelemetryEventKind::ClaimReleased,
    TelemetryEventKind::GateEvaluated,
    TelemetryEventKind::ToolCall,
    TelemetryEventKind::ModelInvocation,
    TelemetryEventKind::VerificationRun,
    TelemetryEventKind::ConflictDetected,
    TelemetryEventKind::HumanHandoff,
];

fn deserialize_sampling_rate<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    match u32::try_from(value) {
        Ok(rate) if value <= MAX_SAMPLING_RATE_PER_MYRIAD => Ok(rate),
        _ => Err(D::Error::custom(format!(
            "sampling.rate must be in the inclusive range 0..=10000; got {value}"
        ))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TelemetryContractDocument {
    pub schema_version: String,
    pub telemetry_contract: TelemetryContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TelemetryContract {
    pub id: StableId,
    pub enabled: bool,
    pub sink: TelemetrySink,
    pub events: Vec<TelemetryEventSpec>,
    pub sampling: SamplingPolicy,
    pub privacy: PrivacyPolicy,
    pub correlation: CorrelationPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TelemetrySink {
    JsonlFile,
    OpenTelemetry,
    Stderr,
    Http,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TelemetryEventSpec {
    pub kind: TelemetryEventKind,
    pub record: bool,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventKind {
    PhaseTransition,
    ClaimAcquired,
    ClaimReleased,
    GateEvaluated,
    ToolCall,
    ModelInvocation,
    VerificationRun,
    ConflictDetected,
    HumanHandoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SamplingPolicy {
    /// Per-myriad sampling rate: `10_000` means always eligible, `0` means never
    /// eligible except for `always_record_kinds`.
    #[schemars(range(min = 0, max = 10_000))]
    #[serde(deserialize_with = "deserialize_sampling_rate")]
    pub rate: u32,
    pub max_per_second: Option<u32>,
    pub always_record_kinds: Vec<TelemetryEventKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrivacyPolicy {
    pub redact_secrets: bool,
    pub redact_paths: bool,
    pub hash_agent_ids: bool,
    pub denylist_field_globs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorrelationPolicy {
    pub trace_parent: Option<String>,
    pub run_id_ref: Option<StableId>,
    pub span_id_seed: Option<String>,
}

impl TelemetryContract {
    /// Returns whether an event kind is eligible to be recorded by this manifest.
    ///
    /// This helper is deterministic by design: it does not perform random sampling.
    /// A configured event with `record: true` is treated as eligible when
    /// `sampling.rate > 0`; `always_record_kinds` bypass the rate check.
    #[must_use]
    pub fn should_record(&self, kind: TelemetryEventKind) -> bool {
        if !self.enabled {
            return false;
        }

        if self.sampling.always_record_kinds.contains(&kind) {
            return true;
        }

        self.sampling.rate > 0
            && self
                .events
                .iter()
                .any(|event| event.kind == kind && event.record)
    }

    /// Iterates over every known event kind that this manifest would record.
    pub fn recordable_kinds(&self) -> impl Iterator<Item = TelemetryEventKind> + '_ {
        ALL_TELEMETRY_EVENT_KINDS
            .iter()
            .copied()
            .filter(move |kind| self.should_record(*kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract(enabled: bool) -> TelemetryContract {
        TelemetryContract {
            id: StableId("telemetry.default".into()),
            enabled,
            sink: TelemetrySink::JsonlFile,
            events: vec![
                TelemetryEventSpec {
                    kind: TelemetryEventKind::ToolCall,
                    record: true,
                    fields: vec!["tool".into(), "duration_ms".into()],
                },
                TelemetryEventSpec {
                    kind: TelemetryEventKind::ModelInvocation,
                    record: false,
                    fields: vec!["model".into(), "tokens".into()],
                },
            ],
            sampling: SamplingPolicy {
                rate: 10_000,
                max_per_second: Some(50),
                always_record_kinds: vec![TelemetryEventKind::GateEvaluated],
            },
            privacy: PrivacyPolicy {
                redact_secrets: true,
                redact_paths: false,
                hash_agent_ids: true,
                denylist_field_globs: vec!["*.secret".into()],
            },
            correlation: CorrelationPolicy {
                trace_parent: Some(
                    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-00".into(),
                ),
                run_id_ref: Some(StableId("run.wave-4".into())),
                span_id_seed: Some("seed".into()),
            },
        }
    }

    #[test]
    fn telemetry_contract_round_trips_through_yaml() {
        let doc = TelemetryContractDocument {
            schema_version: "0.1".into(),
            telemetry_contract: sample_contract(true),
        };

        let yaml = serde_yaml::to_string(&doc).expect("serialize telemetry contract");
        let back: TelemetryContractDocument =
            serde_yaml::from_str(&yaml).expect("deserialize telemetry contract");

        assert_eq!(doc, back);
    }

    #[test]
    fn example_telemetry_yaml_round_trips() {
        let yaml = include_str!("../../../contracts/examples/telemetry.yaml");
        let doc: TelemetryContractDocument =
            serde_yaml::from_str(yaml).expect("deserialize telemetry example");
        let serialized = serde_yaml::to_string(&doc).expect("serialize telemetry example");
        let reparsed: TelemetryContractDocument =
            serde_yaml::from_str(&serialized).expect("deserialize serialized telemetry example");

        assert_eq!(doc, reparsed);
    }

    #[test]
    fn telemetry_contract_rejects_unknown_fields() {
        let yaml = r#"
schema_version: "0.1"
telemetry_contract:
  id: "telemetry.default"
  enabled: true
  sink: "jsonl_file"
  events: []
  sampling:
    rate: 10000
    max_per_second: null
    always_record_kinds: []
  privacy:
    redact_secrets: true
    redact_paths: false
    hash_agent_ids: true
    denylist_field_globs: []
  correlation:
    trace_parent: null
    run_id_ref: null
    span_id_seed: null
  extra: "not allowed"
"#;

        let err = serde_yaml::from_str::<TelemetryContractDocument>(yaml).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_sampling_rate_above_10_000() {
        let yaml = include_str!("../../../contracts/examples/telemetry.yaml").replacen(
            "rate: 10000",
            "rate: 10001",
            1,
        );

        let err = serde_yaml::from_str::<TelemetryContractDocument>(&yaml).unwrap_err();

        assert!(err.to_string().contains("sampling.rate"));
    }

    #[test]
    fn should_record_honors_enabled_always_record_and_record_specs() {
        let contract = sample_contract(true);

        assert!(contract.should_record(TelemetryEventKind::GateEvaluated));
        assert!(contract.should_record(TelemetryEventKind::ToolCall));
        assert!(!contract.should_record(TelemetryEventKind::ModelInvocation));
        assert!(!contract.should_record(TelemetryEventKind::ClaimAcquired));

        let disabled = sample_contract(false);
        assert!(!disabled.should_record(TelemetryEventKind::GateEvaluated));
        assert!(!disabled.should_record(TelemetryEventKind::ToolCall));
    }

    #[test]
    fn should_record_treats_zero_rate_as_never_except_always_record_kinds() {
        let mut contract = sample_contract(true);
        contract.sampling.rate = 0;

        assert!(contract.should_record(TelemetryEventKind::GateEvaluated));
        assert!(!contract.should_record(TelemetryEventKind::ToolCall));
    }

    #[test]
    fn recordable_kinds_returns_recorded_subset() {
        let contract = sample_contract(true);

        let kinds: Vec<_> = contract.recordable_kinds().collect();

        assert_eq!(
            kinds,
            vec![
                TelemetryEventKind::GateEvaluated,
                TelemetryEventKind::ToolCall,
            ]
        );
    }
}
