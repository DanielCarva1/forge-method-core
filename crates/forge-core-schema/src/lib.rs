use forge_core_contracts::{
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument,
    CompletionContractDocument, ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument,
    OperationCrossReferencePolicyDocument, RequestContractDocument, RuntimeCapabilityDocument,
    RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument, ToolEffectContractDocument,
    WorkflowGovernanceBundleDocument, WorkflowGovernanceEvaluationDocument,
    WorkflowGovernanceLedgerDocument, WorkflowGovernanceReceiptDocument,
    WorkflowMigrationPlanDocument,
};
use schemars::{schema_for, JsonSchema};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContractSchemaArtifact {
    pub family_id: &'static str,
    pub document_type: &'static str,
    pub root_key: Option<&'static str>,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactAgentView {
    pub family_id: &'static str,
    pub document_type: &'static str,
    pub root_key: Option<&'static str>,
    pub top_level_required_fields: Vec<String>,
    pub contract_required_fields: Vec<String>,
    pub enum_definitions: Vec<String>,
    pub authority_note: &'static str,
}

#[must_use]
// Keep the complete schema registry visible in one auditable table until the
// planned ContractFamilyRegistry replaces these manual registrations.
#[allow(clippy::too_many_lines)]
pub fn generated_contract_schemas() -> Vec<ContractSchemaArtifact> {
    vec![
        schema_artifact::<FieldEvidenceRegistry>(
            "field_evidence_registry",
            "FieldEvidenceRegistry",
            None,
            "research evidence registry; supports decisions but is not runtime authority",
        ),
        schema_artifact::<ContractFamilyInventoryDocument>(
            "contract_family_inventory",
            "ContractFamilyInventoryDocument",
            Some("contract_family_inventory"),
            "inventory lock for contract family coverage and code generation",
        ),
        schema_artifact::<CommandContractDocument>(
            "command_contract",
            "CommandContractDocument",
            Some("command_contract"),
            "schema-backed command surface; shell prose is not authority",
        ),
        schema_artifact::<OperationContractDocument>(
            "operation_contract",
            "OperationContractDocument",
            Some("operation_contract"),
            "central authority response for host-agent action",
        ),
        schema_artifact::<OperationCrossReferencePolicyDocument>(
            "operation_reference_policy",
            "OperationCrossReferencePolicyDocument",
            None,
            "approved cross-contract (side-contract) reference fields for operations; not citation refs",
        ),
        schema_artifact::<ClaimContractDocument>(
            "claim_contract",
            "ClaimContractDocument",
            Some("claim_contract"),
            "ownership and lease boundary; not health diagnosis",
        ),
        schema_artifact::<CompletionContractDocument>(
            "completion_contract",
            "CompletionContractDocument",
            Some("completion_contract"),
            "done-state and proof boundary; prevents duplicate work",
        ),
        schema_artifact::<GateContractDocument>(
            "gate_contract",
            "GateContractDocument",
            Some("gate_contract"),
            "validation scope boundary for lane/product/integration/release",
        ),
        schema_artifact::<RequestContractDocument>(
            "request_contract",
            "RequestContractDocument",
            Some("request_contract"),
            "durable coordination message; cannot mutate integration state directly",
        ),
        schema_artifact::<ToolEffectContractDocument>(
            "tool_effect_contract",
            "ToolEffectContractDocument",
            Some("tool_effect_contract"),
            "tool side-effect safety, read/write, conflict, inverse, and repair boundary",
        ),
        schema_artifact::<DecisionCloseContractDocument>(
            "decision_close_contract",
            "DecisionCloseContractDocument",
            Some("decision_close_contract"),
            "decision close authority; chat momentum is not close evidence",
        ),
        schema_artifact::<RuntimeHandoffContractDocument>(
            "runtime_handoff_contract",
            "RuntimeHandoffContractDocument",
            Some("runtime_handoff_contract"),
            "adapter-agnostic runtime handoff with registry/capability double gate",
        ),
        schema_artifact::<RuntimeRegistryEntryDocument>(
            "runtime_registry_entry",
            "RuntimeRegistryEntryDocument",
            Some("runtime_registry_entry"),
            "runtime identity and trust policy evidence for handoff",
        ),
        schema_artifact::<RuntimeCapabilityDocument>(
            "runtime_capability",
            "RuntimeCapabilityDocument",
            Some("runtime_capability"),
            "schema-backed capability evidence for handoff",
        ),
        schema_artifact::<HealthRecoveryContractDocument>(
            "health_recovery_contract",
            "HealthRecoveryContractDocument",
            Some("health_recovery_contract"),
            "runtime health/anomaly/recovery intent; not daemon topology",
        ),
        schema_artifact::<CoordinationEvalContractDocument>(
            "coordination_eval_contract",
            "CoordinationEvalContractDocument",
            Some("coordination_eval_contract"),
            "coordination failure-mode eval surface; schema shape is not readiness",
        ),
        schema_artifact::<AssuranceCaseDocument>(
            "assurance_case",
            "AssuranceCaseDocument",
            Some("assurance_case"),
            "evidence-backed obligations and target-specific readiness; agent confidence is not authority",
        ),
        schema_artifact::<WorkflowMigrationPlanDocument>(
            "workflow_migration_plan",
            "WorkflowMigrationPlanDocument",
            Some("workflow_migration_plan"),
            "read-only migration classification policy; does not authorize execution or retirement",
        ),
        schema_artifact::<WorkflowGovernanceBundleDocument>(
            "workflow_governance_bundle",
            "WorkflowGovernanceBundleDocument",
            Some("workflow_governance_bundle"),
            "governance policy; raw-document evaluation produces simulation-only candidate guidance",
        ),
        schema_artifact::<WorkflowGovernanceEvaluationDocument>(
            "workflow_governance_evaluation",
            "WorkflowGovernanceEvaluationDocument",
            Some("workflow_governance_evaluation"),
            "untrusted observation proposal; output cannot create authority, completion, progression, or mutation",
        ),
        schema_artifact::<WorkflowGovernanceLedgerDocument>(
            "workflow_governance_ledger",
            "WorkflowGovernanceLedgerDocument",
            Some("workflow_governance_ledger"),
            "hash-chained durable receipt history; trusted only after strict store recovery and adapter binding",
        ),
        schema_artifact::<WorkflowGovernanceReceiptDocument>(
            "workflow_governance_receipt",
            "WorkflowGovernanceReceiptDocument",
            Some("workflow_governance_receipt"),
            "single hash-chained governance receipt; serialization alone is not authority",
        ),
    ]
}

#[must_use]
pub fn compact_agent_views() -> Vec<CompactAgentView> {
    generated_contract_schemas()
        .into_iter()
        .map(|artifact| CompactAgentView {
            family_id: artifact.family_id,
            document_type: artifact.document_type,
            root_key: artifact.root_key,
            top_level_required_fields: required_fields(&artifact.schema),
            contract_required_fields: artifact.root_key.map_or_else(
                || required_fields(&artifact.schema),
                |root_key| required_fields_for_property(&artifact.schema, root_key),
            ),
            enum_definitions: enum_definitions(&artifact.schema),
            authority_note: authority_note(artifact.family_id),
        })
        .collect()
}

fn schema_artifact<T>(
    family_id: &'static str,
    document_type: &'static str,
    root_key: Option<&'static str>,
    authority_note: &'static str,
) -> ContractSchemaArtifact
where
    T: JsonSchema,
{
    let mut schema = serde_json::to_value(schema_for!(T)).expect("schema serializes to json");
    schema["x-forge-family-id"] = Value::String(family_id.to_string());
    schema["x-forge-authority-note"] = Value::String(authority_note.to_string());
    ContractSchemaArtifact {
        family_id,
        document_type,
        root_key,
        schema,
    }
}

fn required_fields(schema: &Value) -> Vec<String> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn required_fields_for_property(schema: &Value, property: &str) -> Vec<String> {
    let Some(property_schema) = schema
        .get("properties")
        .and_then(|properties| properties.get(property))
    else {
        return Vec::new();
    };

    if let Some(required) = property_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
    {
        return required;
    }

    property_schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| resolve_local_ref(schema, reference))
        .map(required_fields)
        .unwrap_or_default()
}

fn resolve_local_ref<'a>(schema: &'a Value, reference: &str) -> Option<&'a Value> {
    let path = reference.strip_prefix("#/$defs/")?;
    schema.get("$defs")?.get(path)
}

fn enum_definitions(schema: &Value) -> Vec<String> {
    let mut names = Vec::new();
    collect_enum_definitions(schema, &mut names);
    names.sort();
    names.dedup();
    names
}

fn collect_enum_definitions(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("enum").and_then(Value::as_array).is_some() {
                if let Some(title) = map.get("title").and_then(Value::as_str) {
                    names.push(title.to_owned());
                }
            }
            for child in map.values() {
                collect_enum_definitions(child, names);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_enum_definitions(item, names);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn authority_note(family_id: &str) -> &'static str {
    match family_id {
        "field_evidence_registry" => {
            "supports decisions; does not authorize runtime mutation by itself"
        }
        "operation_contract" => {
            "host agents follow operation authority; they do not invent actions"
        }
        "request_contract" => {
            "append-only coordination; driver/runtime authority applies state changes"
        }
        "tool_effect_contract" => "mutating effects need read/write, conflict, and repair policy",
        "decision_close_contract" => "phase/spec/release close requires explicit evidence",
        "runtime_handoff_contract" => "handoff requires registry and capability evidence",
        "health_recovery_contract" => "recovery intent requires claim/request/evidence refs",
        "coordination_eval_contract" => "coordination readiness requires failure-mode coverage",
        "assurance_case" => {
            "readiness requires coherent obligations, claims, evidence, blockers, and waivers"
        }
        "workflow_migration_plan" => {
            "classification is read-only; runtime mutation and retirement remain forbidden"
        }
        "workflow_governance_bundle" => {
            "raw policy evaluation is simulation-only; verified use requires an opaque trusted kernel snapshot"
        }
        "workflow_governance_evaluation" => {
            "caller observations are untrusted proposals; candidate completion is not authority"
        }
        "workflow_governance_ledger" => {
            "strict hash-chain recovery plus project and admitted-bundle binding are required"
        }
        "workflow_governance_receipt" => {
            "serialized receipts require chain, freshness, subject, scope, and revocation validation"
        }
        _ => "generated view; Rust validation remains authority",
    }
}
