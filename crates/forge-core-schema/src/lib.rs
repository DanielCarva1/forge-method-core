use forge_core_contracts::{
    AssuranceCaseDocument, ClaimContractDocument, CommandContractDocument,
    CompletionContractDocument, ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, DomainPackActivePointerDocument,
    DomainPackCapabilitySandboxPolicyDocument, DomainPackCompatibilityReportDocument,
    DomainPackCompositionProjectionDocument, DomainPackCompositionRequestDocument,
    DomainPackContentDocument, DomainPackExactLockDocument, DomainPackLifecycleLedgerDocument,
    DomainPackLifecyclePreflightDocument, DomainPackLifecycleReceiptDocument,
    DomainPackLifecycleRequestDocument, DomainPackManifestDocument,
    DomainPackProjectRequirementsDocument, DomainPackRecoveryReportDocument,
    DomainPackResolutionProjectionDocument, DomainPackResolutionRequestDocument,
    DomainPackRuntimeCapabilityRegistryDocument, DomainPackSupplyChainRegistryDocument,
    DomainPackTrustPolicyDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument,
    OperationCrossReferencePolicyDocument, RequestContractDocument, RuntimeCapabilityDocument,
    RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument, ToolEffectContractDocument,
    WorkflowBehavioralCorpusSetDocument, WorkflowBehavioralCoveragePolicyDocument,
    WorkflowBehavioralReviewSubjectDocument, WorkflowBehavioralScenarioCorpusDocument,
    WorkflowBehavioralShadowReportDocument, WorkflowConsumerCompatibilityMatrixDocument,
    WorkflowConsumerCompatibilityReportDocument, WorkflowDeletionProofDocument,
    WorkflowFinalScorecardDocument, WorkflowGovernanceBundleDocument,
    WorkflowGovernanceEvaluationDocument, WorkflowGovernanceLedgerDocument,
    WorkflowGovernancePolicyOverlayDocument, WorkflowGovernanceReceiptDocument,
    WorkflowGovernanceReleaseManifestDocument, WorkflowMigrationBatchDocument,
    WorkflowMigrationPlanDocument, WorkflowReleaseAdmissionAuthorizationDocument,
    WorkflowReleaseReviewIndexDocument, WorkflowReleaseReviewerRegistryDocument,
    WorkflowRetirementAuthorizationDocument, WorkflowRetirementAuthorizationV2Document,
    WorkflowRetirementEvidenceIndexDocument, WorkflowRetirementTombstoneCatalogDocument,
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
        schema_artifact::<DomainPackManifestDocument>(
            "domain_pack_manifest",
            "DomainPackManifestDocument",
            Some("domain_pack_manifest"),
            "content-addressed candidate metadata only; provenance is not trust, installation, activation, or mutation authority",
        ),
        schema_artifact::<DomainPackContentDocument>(
            "domain_pack_content",
            "DomainPackContentDocument",
            Some("domain_pack_content"),
            "closed candidate knowledge only; prose, playbooks, capabilities, evaluators, and adapters remain untrusted declarations",
        ),
        schema_artifact::<DomainPackProjectRequirementsDocument>(
            "domain_pack_project_requirements",
            "DomainPackProjectRequirementsDocument",
            Some("domain_pack_project_requirements"),
            "durable desired domain surface independent of installed packs; absence must remain an explicit gap",
        ),
        schema_artifact::<DomainPackCompositionRequestDocument>(
            "domain_pack_composition_request",
            "DomainPackCompositionRequestDocument",
            Some("domain_pack_composition_request"),
            "read-only candidate composition input; it cannot shadow core, install packs, or authorize mutation",
        ),
        schema_artifact::<DomainPackCompositionProjectionDocument>(
            "domain_pack_composition_projection",
            "DomainPackCompositionProjectionDocument",
            Some("domain_pack_composition_projection"),
            "derived candidate-only projection; composable status and digest do not admit runtime authority",
        ),
        schema_artifact::<DomainPackTrustPolicyDocument>(
            "domain_pack_trust_policy",
            "DomainPackTrustPolicyDocument",
            Some("domain_pack_trust_policy"),
            "candidate policy input only; authored trust rules and keys require protected-path loading and cryptographic verification",
        ),
        schema_artifact::<DomainPackSupplyChainRegistryDocument>(
            "domain_pack_supply_chain_registry",
            "DomainPackSupplyChainRegistryDocument",
            Some("domain_pack_supply_chain_registry"),
            "candidate registry snapshot; records, grants, revocations, and signatures create no authority until trusted verification",
        ),
        schema_artifact::<DomainPackRuntimeCapabilityRegistryDocument>(
            "domain_pack_runtime_capability_registry",
            "DomainPackRuntimeCapabilityRegistryDocument",
            Some("domain_pack_runtime_capability_registry"),
            "operator-owned capability evidence only; declarations do not prove availability or execution permission",
        ),
        schema_artifact::<DomainPackCapabilitySandboxPolicyDocument>(
            "domain_pack_capability_sandbox_policy",
            "DomainPackCapabilitySandboxPolicyDocument",
            Some("domain_pack_capability_sandbox_policy"),
            "default-deny sandbox input; only exact verified core built-ins may be considered for activation",
        ),
        schema_artifact::<DomainPackResolutionRequestDocument>(
            "domain_pack_resolution_request",
            "DomainPackResolutionRequestDocument",
            Some("domain_pack_resolution_request"),
            "candidate-only deterministic resolution input; it cannot install, activate, or mint namespace ownership",
        ),
        schema_artifact::<DomainPackResolutionProjectionDocument>(
            "domain_pack_resolution_projection",
            "DomainPackResolutionProjectionDocument",
            Some("domain_pack_resolution_projection"),
            "derived candidate selection only; resolved status is not lifecycle admission or runtime authority",
        ),
        schema_artifact::<DomainPackExactLockDocument>(
            "domain_pack_exact_lock",
            "DomainPackExactLockDocument",
            Some("domain_pack_exact_lock"),
            "content-addressed lifecycle proposal; only a trusted crash-safe commit may make a generation active",
        ),
        schema_artifact::<DomainPackCompatibilityReportDocument>(
            "domain_pack_compatibility_report",
            "DomainPackCompatibilityReportDocument",
            Some("domain_pack_compatibility_report"),
            "candidate compatibility evidence; authored status cannot waive requirements, trust, sandbox, or core invariants",
        ),
        schema_artifact::<DomainPackLifecycleRequestDocument>(
            "domain_pack_lifecycle_request",
            "DomainPackLifecycleRequestDocument",
            Some("domain_pack_lifecycle_request"),
            "lifecycle intent only; expected-state CAS, retained lock, preflight, and trusted kernel commit remain mandatory",
        ),
        schema_artifact::<DomainPackLifecyclePreflightDocument>(
            "domain_pack_lifecycle_preflight",
            "DomainPackLifecyclePreflightDocument",
            Some("domain_pack_lifecycle_preflight"),
            "fresh candidate preflight evidence; a clean label cannot perform or authorize mutation",
        ),
        schema_artifact::<DomainPackActivePointerDocument>(
            "domain_pack_active_pointer",
            "DomainPackActivePointerDocument",
            Some("domain_pack_active_pointer"),
            "content-addressed active-generation pointer trusted only after strict store recovery and ledger verification",
        ),
        schema_artifact::<DomainPackLifecycleLedgerDocument>(
            "domain_pack_lifecycle_ledger",
            "DomainPackLifecycleLedgerDocument",
            Some("domain_pack_lifecycle_ledger"),
            "hash-chained lifecycle history; serialization alone cannot prove an operation committed",
        ),
        schema_artifact::<DomainPackLifecycleReceiptDocument>(
            "domain_pack_lifecycle_receipt",
            "DomainPackLifecycleReceiptDocument",
            Some("domain_pack_lifecycle_receipt"),
            "commit receipt evidence requiring exact pointer, lock, preflight, and ledger-chain verification",
        ),
        schema_artifact::<DomainPackRecoveryReportDocument>(
            "domain_pack_recovery_report",
            "DomainPackRecoveryReportDocument",
            Some("domain_pack_recovery_report"),
            "candidate recovery assessment only; ambiguous state must fail closed at the trusted store boundary",
        ),
        schema_artifact::<WorkflowMigrationPlanDocument>(
            "workflow_migration_plan",
            "WorkflowMigrationPlanDocument",
            Some("workflow_migration_plan"),
            "read-only migration classification policy; does not authorize execution or retirement",
        ),
        schema_artifact::<WorkflowGovernancePolicyOverlayDocument>(
            "workflow_governance_policy_overlay",
            "WorkflowGovernancePolicyOverlayDocument",
            Some("workflow_governance_policy_overlay"),
            "non-authoritative candidate policy contribution; only deterministic composition and later trusted admission may make a policy executable",
        ),
        schema_artifact::<WorkflowBehavioralReviewSubjectDocument>(
            "workflow_behavioral_review_subject",
            "WorkflowBehavioralReviewSubjectDocument",
            Some("workflow_behavioral_review_subject"),
            "acyclic candidate-only review subject; it excludes evidence and cannot admit a release",
        ),
        schema_artifact::<WorkflowBehavioralCoveragePolicyDocument>(
            "workflow_behavioral_coverage_policy",
            "WorkflowBehavioralCoveragePolicyDocument",
            Some("workflow_behavioral_coverage_policy"),
            "non-authoritative shadow-coverage requirements; authored thresholds and passing labels cannot create admission",
        ),
        schema_artifact::<WorkflowBehavioralScenarioCorpusDocument>(
            "workflow_behavioral_scenario_corpus",
            "WorkflowBehavioralScenarioCorpusDocument",
            Some("workflow_behavioral_scenario_corpus"),
            "non-authoritative typed behavioral scenarios; expected projections require deterministic recomputation and independent review",
        ),
        schema_artifact::<WorkflowBehavioralCorpusSetDocument>(
            "workflow_behavioral_corpus_set",
            "WorkflowBehavioralCorpusSetDocument",
            Some("workflow_behavioral_corpus_set"),
            "content-addressed non-authoritative corpus inventory; aggregation is not evidence validity or release admission",
        ),
        schema_artifact::<WorkflowBehavioralShadowReportDocument>(
            "workflow_behavioral_shadow_report",
            "WorkflowBehavioralShadowReportDocument",
            Some("workflow_behavioral_shadow_report"),
            "non-authoritative consistency candidate only; a shadow pass cannot grant executable, completion, retirement, or release authority",
        ),
        schema_artifact::<WorkflowReleaseReviewIndexDocument>(
            "workflow_release_review_index",
            "WorkflowReleaseReviewIndexDocument",
            Some("workflow_release_review_index"),
            "candidate-only independent-review index; artifact bindings and review decisions cannot admit a runtime release",
        ),
        schema_artifact::<WorkflowReleaseReviewerRegistryDocument>(
            "workflow_release_reviewer_registry",
            "WorkflowReleaseReviewerRegistryDocument",
            Some("workflow_release_reviewer_registry"),
            "candidate-only credential registry input; trusted status, key, role, time, and independence verification is required",
        ),
        schema_artifact::<WorkflowReleaseAdmissionAuthorizationDocument>(
            "workflow_release_admission_authorization",
            "WorkflowReleaseAdmissionAuthorizationDocument",
            Some("workflow_release_admission_authorization"),
            "candidate authorization envelope only; deserialization or signatures without trusted verification cannot admit runtime authority",
        ),
        schema_artifact::<WorkflowGovernanceReleaseManifestDocument>(
            "workflow_governance_release_manifest",
            "WorkflowGovernanceReleaseManifestDocument",
            Some("workflow_governance_release_manifest"),
            "versioned rollout intent only; deserialization does not admit executable or retired workflow authority",
        ),
        schema_artifact::<WorkflowMigrationBatchDocument>(
            "workflow_migration_batch",
            "WorkflowMigrationBatchDocument",
            Some("workflow_migration_batch"),
            "candidate policy batch only; trusted composition and admission are required before executable use",
        ),
        schema_artifact::<WorkflowRetirementAuthorizationDocument>(
            "workflow_retirement_authorization",
            "WorkflowRetirementAuthorizationDocument",
            Some("workflow_retirement_authorization"),
            "retirement proposal requires trusted binding and signature verification; deserialization is not authority",
        ),
        schema_artifact::<WorkflowRetirementEvidenceIndexDocument>(
            "workflow_retirement_evidence_index",
            "WorkflowRetirementEvidenceIndexDocument",
            Some("workflow_retirement_evidence_index"),
            "candidate-only exact retirement/evidence inventory; authored bindings cannot retire legacy authority",
        ),
        schema_artifact::<WorkflowDeletionProofDocument>(
            "workflow_deletion_proof",
            "WorkflowDeletionProofDocument",
            Some("workflow_deletion_proof"),
            "candidate-only ablation result; authored equality requires deterministic recomputation",
        ),
        schema_artifact::<WorkflowConsumerCompatibilityReportDocument>(
            "workflow_consumer_compatibility_report",
            "WorkflowConsumerCompatibilityReportDocument",
            Some("workflow_consumer_compatibility_report"),
            "candidate-only compatibility-window observations; authored counts cannot authorize retirement",
        ),
        schema_artifact::<WorkflowConsumerCompatibilityMatrixDocument>(
            "workflow_consumer_compatibility_matrix",
            "WorkflowConsumerCompatibilityMatrixDocument",
            Some("workflow_consumer_compatibility_matrix"),
            "candidate-only repository fixture matrix; it is not runtime telemetry or retirement authority",
        ),
        schema_artifact::<WorkflowRetirementTombstoneCatalogDocument>(
            "workflow_retirement_tombstone_catalog",
            "WorkflowRetirementTombstoneCatalogDocument",
            Some("workflow_retirement_tombstone_catalog"),
            "non-authoritative diagnostics only; tombstones cannot route, execute, or retire workflows",
        ),
        schema_artifact::<WorkflowFinalScorecardDocument>(
            "workflow_final_scorecard",
            "WorkflowFinalScorecardDocument",
            Some("workflow_final_scorecard"),
            "derived candidate-only two-axis scorecard; authored counts are not retirement authority",
        ),
        schema_artifact::<WorkflowRetirementAuthorizationV2Document>(
            "workflow_retirement_authorization_v2",
            "WorkflowRetirementAuthorizationV2Document",
            Some("workflow_retirement_authorization_v2"),
            "aggregate candidate authorization; trusted independent signature verification is required",
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
            if let Some(definitions) = map.get("$defs").and_then(Value::as_object) {
                for (name, definition) in definitions {
                    if contains_enum(definition) {
                        names.push(name.clone());
                    }
                }
            }
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

fn contains_enum(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            map.get("enum").and_then(Value::as_array).is_some() || map.values().any(contains_enum)
        }
        Value::Array(items) => items.iter().any(contains_enum),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

// The exhaustive authority map intentionally stays beside schema generation so
// every published family has one auditable, non-authority statement.
#[allow(clippy::too_many_lines)]
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
        "domain_pack_manifest" => {
            "candidate-only content identity and source metadata; provenance is not trust or activation"
        }
        "domain_pack_content" => {
            "candidate-only knowledge; prose and declarations cannot become prompts, tool arguments, or authority"
        }
        "domain_pack_project_requirements" => {
            "requirements persist independently of packs so removal produces an explicit gap"
        }
        "domain_pack_composition_request" => {
            "candidate-only read surface; composition cannot mutate core, install packs, or grant authority"
        }
        "domain_pack_composition_projection" => {
            "derived candidate-only result; composable status requires later trusted lifecycle admission"
        }
        "domain_pack_trust_policy" => {
            "authored trust policy is candidate input; protected loading and cryptographic verification remain mandatory"
        }
        "domain_pack_supply_chain_registry" => {
            "registry declarations cannot self-grant namespace ownership or supply-chain assurance"
        }
        "domain_pack_runtime_capability_registry" => {
            "exact package, subject, provider, implementation, status, and evidence checks are required for operator-owned bindings"
        }
        "domain_pack_capability_sandbox_policy" => {
            "default deny remains authority; declarations cannot authorize external execution"
        }
        "domain_pack_resolution_request" => {
            "resolution input is candidate-only and cannot install or activate a pack"
        }
        "domain_pack_resolution_projection" => {
            "resolved selection remains candidate evidence pending trust, composition, capability, compatibility, and lifecycle admission"
        }
        "domain_pack_exact_lock" => {
            "exact lock is content-addressed intent; only trusted crash-safe commit can activate its generation"
        }
        "domain_pack_compatibility_report" => {
            "compatibility is derived candidate evidence and cannot waive persistent requirements or core invariants"
        }
        "domain_pack_lifecycle_request" => {
            "lifecycle intent requires expected-state CAS and cannot mutate state by deserialization"
        }
        "domain_pack_lifecycle_preflight" => {
            "clean preflight is short-lived candidate evidence, not commit authority"
        }
        "domain_pack_active_pointer" => {
            "active pointer is trusted only after strict recovery, digest, lock, and ledger verification"
        }
        "domain_pack_lifecycle_ledger" => {
            "hash-chain verification and strict store recovery are required before ledger records are authoritative"
        }
        "domain_pack_lifecycle_receipt" => {
            "receipt serialization alone cannot prove a lifecycle transaction committed"
        }
        "domain_pack_recovery_report" => {
            "recovery assessment is candidate-only; ambiguous state must remain blocked"
        }
        "workflow_migration_plan" => {
            "classification is read-only; runtime mutation and retirement remain forbidden"
        }
        "workflow_governance_policy_overlay" => {
            "non-authoritative candidate-only overlay; cannot become executable without deterministic composition and trusted admission"
        }
        "workflow_behavioral_review_subject" => {
            "acyclic non-authoritative candidate-only review subject; excludes evidence and cannot admit a release"
        }
        "workflow_behavioral_coverage_policy" => {
            "non-authoritative coverage floor; authored thresholds or pass labels cannot create authority"
        }
        "workflow_behavioral_scenario_corpus" => {
            "typed shadow scenarios are non-authoritative and cannot replace deterministic recomputation"
        }
        "workflow_behavioral_corpus_set" => {
            "non-authoritative content-addressed corpus aggregation is not evidence validity or admission"
        }
        "workflow_behavioral_shadow_report" => {
            "non-authoritative shadow consistency is candidate-only; it cannot grant executable or release authority"
        }
        "workflow_release_review_index" => {
            "candidate-only review index; exact artifact bindings and review decisions cannot admit runtime authority"
        }
        "workflow_release_reviewer_registry" => {
            "candidate-only credential registry; trusted key, role, status, time, and independence verification is required"
        }
        "workflow_release_admission_authorization" => {
            "candidate authorization only; trusted cryptographic and semantic verification is required before admission"
        }
        "workflow_governance_release_manifest" => {
            "manifest entries are rollout intent only; executable and retired states require trusted derived admission"
        }
        "workflow_migration_batch" => {
            "batch policies are candidate-only; deserialization and presence never grant executable authority"
        }
        "workflow_retirement_authorization" => {
            "retirement requires trusted evidence binding and signature verification; deserialization is not authority"
        }
        "workflow_retirement_evidence_index" => {
            "candidate-only exact evidence index; bindings cannot retire legacy authority"
        }
        "workflow_deletion_proof" => {
            "candidate-only deletion comparison; authored equality cannot retire legacy authority"
        }
        "workflow_consumer_compatibility_report" => {
            "candidate-only window observations; authored counts cannot retire legacy authority"
        }
        "workflow_consumer_compatibility_matrix" => {
            "candidate-only repository fixture matrix; it is not telemetry or retirement authority"
        }
        "workflow_retirement_tombstone_catalog" => {
            "non-authoritative diagnostics only; tombstones cannot route or execute workflows"
        }
        "workflow_final_scorecard" => {
            "derived candidate-only two-axis view; authored counts cannot create retired authority"
        }
        "workflow_retirement_authorization_v2" => {
            "candidate authorization only; trusted two-role signature verification is required"
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
