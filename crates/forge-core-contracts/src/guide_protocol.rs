use crate::{GuideDecision, OperationContractDocument};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const GUIDE_PROTOCOL_SCHEMA_VERSION: &str = "0.1";

/// Closed guide input that binds a validated recommendation to the exact
/// `OperationContract` the host must render or execute next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuideProtocolDocument {
    pub schema_version: String,
    pub guide_protocol: GuideProtocol,
}

/// Composition of the existing guide recommendation and operation authority
/// response. Neither side duplicates phase, workflow, action, or execution
/// authority; the kernel validates that both views describe one exact route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuideProtocol {
    pub decision: GuideDecision,
    pub next_operation: OperationContractDocument,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_requires_an_exact_operation_contract() {
        let yaml = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/fixtures/guide-protocol-v0/facilitation.yaml"
        ));
        let document: GuideProtocolDocument =
            yaml_serde::from_str(yaml).expect("deserialize guide protocol");
        assert_eq!(document.schema_version, GUIDE_PROTOCOL_SCHEMA_VERSION);
        assert_eq!(
            document.guide_protocol.decision.recommended_workflow.0,
            "discover-intent"
        );
        assert_eq!(
            document
                .guide_protocol
                .next_operation
                .operation_contract
                .recommendation
                .workflow
                .0,
            "discover-intent"
        );

        let encoded = yaml_serde::to_string(&document).expect("serialize guide protocol");
        let round_trip: GuideProtocolDocument =
            yaml_serde::from_str(&encoded).expect("deserialize round trip");
        assert_eq!(round_trip, document);
    }

    #[test]
    fn protocol_rejects_missing_operation_contract() {
        let yaml = r#"schema_version: "0.1"
guide_protocol:
  decision:
    recommended_workflow: discover-intent
    reason: start
    current_phase: 1-discovery
"#;
        assert!(yaml_serde::from_str::<GuideProtocolDocument>(yaml).is_err());
    }
}
