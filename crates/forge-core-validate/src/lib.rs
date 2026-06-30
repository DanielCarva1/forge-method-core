// The cross-reference and coordination-eval validators walk multiple
// contract fields and accumulate diagnostics per dimension. Splitting them
// to satisfy `clippy::too_many_lines` would scatter related checks across
// helpers and hurt audit readability.
#![allow(clippy::too_many_lines)]

pub mod risk_audit;

use forge_core_contracts::{
    ClaimContractDocument, CommandContractDocument, CompletionContractDocument,
    ContractFamilyInventoryDocument, CoordinationDimension, CoordinationEvalContractDocument,
    CoordinationMetricKind, DecisionCloseContractDocument, DecisionEvidenceKind, DecisionKind,
    DecisionStatus, FieldEvidenceRegistry, GateContractDocument, HealthRecoveryContractDocument,
    HealthStatus, OperationContractDocument, RecoveryAction, RepoPath, RequestContractDocument,
    RuntimeCapabilityDocument, RuntimeHandoffContractDocument, RuntimeHandoffStatus,
    RuntimeRegistryEntryDocument, SourceId, ToolEffectContractDocument,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use yaml_serde::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    #[must_use]
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn extend(&mut self, other: ValidationReport) {
        self.diagnostics.extend(other.diagnostics);
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|item| item.severity == DiagnosticSeverity::Error)
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub path: String,
    pub message: String,
}

impl Diagnostic {
    pub fn error(
        code: DiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn warning(
        code: DiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    YamlReadFailed,
    YamlParseFailed,
    SourceIdMustBeString,
    RepoRefMustBeString,
    MissingKnownRepoRef,
    DuplicateEvidenceSource,
    MissingEvidenceSourceMetadata,
    UnknownEvidenceSourceRef,
    EmptyInventoryFamilies,
    DuplicateInventoryFamily,
    EmptyInventoryFamilyEvidence,
    EmptyInventoryFamilyGlob,
    EmptyCommandArgs,
    UnsafeCommandShellString,
    CommandPublishesWithoutPolicy,
    EmptyOperationStopConditions,
    MissingOperationAuthorityEvidence,
    MutatingOperationMissingEffectRefs,
    FleetOperationMissingAgent,
    FleetOperationMissingRegistry,
    HumanPromptRequiredButEmpty,
    ClaimTtlMustBePositive,
    ClaimHeartbeatIntervalMustBePositive,
    ClaimHandoffPolicyInconsistent,
    CompletionDoneMissingProof,
    CompletionProofRequiredWithoutAcceptedKinds,
    GateBlockingStatusWithoutBlocks,
    RequestMayMutateIntegrationState,
    RequestResponseMismatch,
    RequestAppendOnlyPathMissing,
    EffectReadSetEmpty,
    EffectWriteSetEmpty,
    DestructiveEffectMissingInverseOrStop,
    DestructiveEffectInverseIncomplete,
    DecisionAuthorityEvidenceMissing,
    DecisionClosedMissingTimestamp,
    DecisionClosedMissingGrillEvidence,
    DecisionBlockedMissingReason,
    DecisionBlockedHasTimestamp,
    DecisionPhaseTransitionMissingTargetPhase,
    RuntimeHandoffMissingRegistryEvidence,
    RuntimeHandoffMissingCapabilityEvidence,
    RuntimeHandoffBlockedMissingReason,
    RuntimeHandoffBlockedHasCompleteDoubleGate,
    RuntimeHandoffNonBlockedHasBlockedReason,
    RuntimeHandoffPayloadContextEmpty,
    RuntimeHandoffPayloadAuthorityEmpty,
    RuntimeRegistryCapabilityRefsEmpty,
    RuntimeRegistryUnsafeStateMutationPolicy,
    RuntimeCapabilityOutputEvidenceMissing,
    HealthRecoveryTtlMustBePositive,
    HealthRecoveryAnomalyEvidenceMissing,
    HealthRecoveryUnhealthyWithoutAction,
    HealthRecoveryReviewAllowsAutomaticAction,
    HealthRecoveryReviewMissingRefs,
    HealthRecoveryHandoffContextMissing,
    CoordinationEvalDimensionsEmpty,
    CoordinationEvalDuplicateDimension,
    CoordinationEvalMissingRequiredDimension,
    CoordinationEvalFixtureRefsEmpty,
    CoordinationEvalEvidenceRefsEmpty,
    CoordinationEvalFailureSignalEmpty,
    CoordinationEvalThresholdMissing,
    CoordinationEvalUnexpectedThreshold,
    CoordinationEvalManualReviewPolicyUnsafe,
    MissingReference,
    ReferenceKindMismatch,
    // F11 — Risk Audit (AI-induced anti-patterns).
    RiskAuditRuleMalformed,
    RiskAuditRuleMissingId,
    RiskAuditRuleMissingDetector,
    RiskAuditRuleMissingPattern,
    RiskAuditRuleInvalidSeverity,
    RiskAuditRuleInvalidDetectorKind,
    RiskAuditRuleInvalidAppliesTo,
    RiskAuditRuleMissingFixHint,
    RiskAuditRuleSetEmpty,
    RiskAuditTargetFileUnreadable,
    RiskAuditAntiPatternMatched,
    RiskAuditEvidenceMissing,
    RiskAuditExternalLinterFailed,
    RiskAuditRequiredFileMissing,
}

#[derive(Debug, Clone)]
pub struct ParsedYamlDocument {
    pub path: String,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    ContractDefinition,
    Policy,
    OperationFixture,
    CommandContract,
    ClaimContract,
    CompletionContract,
    GateContract,
    RequestContract,
    ToolEffectContract,
    DecisionCloseContract,
    RuntimeHandoffContract,
    RuntimeRegistryEntry,
    RuntimeCapability,
    HealthRecoveryContract,
    CoordinationEvalContract,
    InventoryContract,
    Ledger,
    RuntimeRegistryProjection,
    EvidenceArtifact,
}

#[derive(Debug, Clone, Default)]
pub struct ReferenceIndex {
    entries: HashMap<String, ReferenceKind>,
}

impl ReferenceIndex {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, reference: impl Into<String>, kind: ReferenceKind) {
        self.entries.insert(reference.into(), kind);
    }

    #[must_use]
    pub fn kind_of(&self, reference: &str) -> Option<ReferenceKind> {
        self.entries.get(reference).copied()
    }

    #[must_use]
    pub fn contains(&self, reference: &str) -> bool {
        self.entries.contains_key(reference)
    }
}

#[must_use]
pub fn validate_evidence_registry(registry: &FieldEvidenceRegistry) -> ValidationReport {
    let mut report = ValidationReport::new();
    let mut seen = HashSet::new();

    for source in &registry.sources {
        if !seen.insert(source.id.0.as_str()) {
            report.push(Diagnostic::error(
                DiagnosticCode::DuplicateEvidenceSource,
                format!("sources.{}", source.id.0),
                "evidence source id must be unique",
            ));
        }
        if source.title.trim().is_empty() || source.url.trim().is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::MissingEvidenceSourceMetadata,
                format!("sources.{}", source.id.0),
                "evidence source requires non-empty title and url",
            ));
        }
    }

    report
}

#[must_use]
pub fn validate_yaml_source_id_references(
    documents: &[ParsedYamlDocument],
    evidence: &FieldEvidenceRegistry,
) -> ValidationReport {
    let known_sources = evidence_source_ids(evidence);
    let mut report = ValidationReport::new();

    for document in documents {
        validate_source_ids_in_value(&mut report, &document.path, &document.value, &known_sources);
    }

    report
}

// Using the default `RandomState` keeps the API simple; downstream callers do
// not need to inject a custom hasher.
#[allow(clippy::implicit_hasher)]
#[must_use]
pub fn validate_yaml_known_repo_references(
    documents: &[ParsedYamlDocument],
    known_paths: &HashSet<String>,
) -> ValidationReport {
    let mut report = ValidationReport::new();

    for document in documents {
        validate_known_refs_in_value(&mut report, &document.path, &document.value, known_paths);
    }

    report
}

fn validate_known_refs_in_value(
    report: &mut ValidationReport,
    document_path: &str,
    value: &Value,
    known_paths: &HashSet<String>,
) {
    match value {
        Value::Mapping(mapping) => {
            for (key, child) in mapping {
                if let Some(key) = key.as_str() {
                    if is_repo_ref_key(key) {
                        validate_ref_like_value(report, document_path, key, child, known_paths);
                    } else {
                        validate_known_refs_in_value(report, document_path, child, known_paths);
                    }
                } else {
                    validate_known_refs_in_value(report, document_path, child, known_paths);
                }
            }
        }
        Value::Sequence(items) => {
            for item in items {
                validate_known_refs_in_value(report, document_path, item, known_paths);
            }
        }
        _ => {}
    }
}

fn validate_ref_like_value(
    report: &mut ValidationReport,
    document_path: &str,
    key: &str,
    value: &Value,
    known_paths: &HashSet<String>,
) {
    match value {
        Value::String(reference) => {
            validate_single_known_repo_ref(report, document_path, key, reference, known_paths);
        }
        Value::Sequence(items) => {
            for item in items {
                match item.as_str() {
                    Some(reference) => {
                        validate_single_known_repo_ref(
                            report,
                            document_path,
                            key,
                            reference,
                            known_paths,
                        );
                    }
                    None => validate_known_refs_in_value(report, document_path, item, known_paths),
                }
            }
        }
        Value::Mapping(_) => {
            validate_known_refs_in_value(report, document_path, value, known_paths);
        }
        _ => {}
    }
}

fn validate_single_known_repo_ref(
    report: &mut ValidationReport,
    document_path: &str,
    key: &str,
    reference: &str,
    known_paths: &HashSet<String>,
) {
    if !is_contracts_or_docs_ref(reference) {
        return;
    }
    let exists = if reference.contains('*') {
        known_paths.contains(glob_parent(reference))
    } else {
        known_paths.contains(reference)
    };
    if !exists {
        report.push(Diagnostic::error(
            DiagnosticCode::MissingKnownRepoRef,
            document_path,
            format!("missing known repo ref {key}={reference}"),
        ));
    }
}

fn is_repo_ref_key(key: &str) -> bool {
    matches!(
        key,
        "ref"
            | "refs"
            | "contract_ref"
            | "schema_ref"
            | "policy_ref"
            | "policy_refs"
            | "supporting_policy_refs"
            | "decision_ref"
            | "fixture_refs"
            | "evidence_refs"
            | "instance_globs"
            | "implementation_refs"
            | "context_refs"
            | "capability_refs"
            | "protocol_refs"
            | "dependency_refs"
    ) || key.ends_with("_ref")
        || key.ends_with("_refs")
}

fn is_contracts_or_docs_ref(reference: &str) -> bool {
    reference.starts_with("contracts/") || reference.starts_with("docs/")
}

fn glob_parent(glob: &str) -> &str {
    let prefix = glob.split('*').next().unwrap_or(glob);
    let trimmed = prefix.trim_end_matches('/');
    if trimmed.is_empty() {
        "."
    } else if prefix.contains('*') || glob.contains('*') {
        trimmed
    } else if let Some((parent, _)) = trimmed.rsplit_once('/') {
        if parent.is_empty() {
            "."
        } else {
            parent
        }
    } else {
        "."
    }
}

fn validate_source_ids_in_value(
    report: &mut ValidationReport,
    document_path: &str,
    value: &Value,
    known_sources: &HashSet<&str>,
) {
    match value {
        Value::Mapping(mapping) => {
            for (key, child) in mapping {
                if key.as_str() == Some("source_id") {
                    match child.as_str() {
                        Some(source_id) if known_sources.contains(source_id) => {}
                        Some(source_id) => report.push(Diagnostic::error(
                            DiagnosticCode::UnknownEvidenceSourceRef,
                            document_path,
                            format!("unknown evidence source_id {source_id}"),
                        )),
                        None => report.push(Diagnostic::error(
                            DiagnosticCode::SourceIdMustBeString,
                            document_path,
                            "source_id must be string",
                        )),
                    }
                }
                validate_source_ids_in_value(report, document_path, child, known_sources);
            }
        }
        Value::Sequence(items) => {
            for item in items {
                validate_source_ids_in_value(report, document_path, item, known_sources);
            }
        }
        _ => {}
    }
}

#[must_use]
pub fn validate_inventory(
    inventory: &ContractFamilyInventoryDocument,
    evidence: &FieldEvidenceRegistry,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    let known_sources = evidence_source_ids(evidence);
    let families = &inventory.contract_family_inventory.families;

    if families.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::EmptyInventoryFamilies,
            "contract_family_inventory.families",
            "inventory must list at least one contract family",
        ));
    }

    let mut seen = HashSet::new();
    for family in families {
        if !seen.insert(family.id.0.as_str()) {
            report.push(Diagnostic::error(
                DiagnosticCode::DuplicateInventoryFamily,
                format!("families.{}", family.id.0),
                "contract family id must be unique",
            ));
        }
        if family.evidence_refs.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::EmptyInventoryFamilyEvidence,
                format!("families.{}.evidence_refs", family.id.0),
                "contract family requires evidence refs",
            ));
        }
        for source_id in &family.evidence_refs {
            check_source_ref(
                &mut report,
                "families.evidence_refs",
                source_id,
                &known_sources,
            );
        }
        for glob in &family.instance_globs {
            if glob.trim().is_empty() {
                report.push(Diagnostic::error(
                    DiagnosticCode::EmptyInventoryFamilyGlob,
                    format!("families.{}.instance_globs", family.id.0),
                    "instance glob must not be empty",
                ));
            }
        }
    }

    for source_id in &inventory.contract_family_inventory.supporting_research_refs {
        check_source_ref(
            &mut report,
            "contract_family_inventory.supporting_research_refs",
            source_id,
            &known_sources,
        );
    }

    report
}

#[must_use]
pub fn validate_inventory_references(
    inventory: &ContractFamilyInventoryDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let inventory = &inventory.contract_family_inventory;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "contract_family_inventory.contract_ref",
        &inventory.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    for policy_ref in &inventory.supporting_policy_refs {
        expect_repo_ref(
            &mut report,
            index,
            "contract_family_inventory.supporting_policy_refs",
            policy_ref,
            &[ReferenceKind::Policy],
        );
    }
    for family in &inventory.families {
        expect_repo_ref(
            &mut report,
            index,
            &format!(
                "contract_family_inventory.families.{}.schema_ref",
                family.id.0
            ),
            &family.schema_ref,
            &[ReferenceKind::ContractDefinition, ReferenceKind::Policy],
        );
    }

    report
}

#[instrument(skip_all, fields(contract_id = %operation.operation_contract.contract_id.0))]
pub fn validate_operation_cross_references(
    operation: &OperationContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let operation = &operation.operation_contract;
    let mut report = ValidationReport::new();

    if let Some(reference) = &operation.coordination_scope.concurrency.registry_ref {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.coordination_scope.concurrency.registry_ref",
            reference,
            &[
                ReferenceKind::RuntimeRegistryEntry,
                ReferenceKind::RuntimeRegistryProjection,
            ],
        );
    }
    if let Some(reference) = &operation
        .coordination_scope
        .write_authority
        .claim_contract_ref
    {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.coordination_scope.write_authority.claim_contract_ref",
            reference,
            &[ReferenceKind::ClaimContract],
        );
    }
    if let Some(reference) = &operation
        .coordination_scope
        .completion
        .completion_contract_ref
    {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.coordination_scope.completion.completion_contract_ref",
            reference,
            &[ReferenceKind::CompletionContract],
        );
    }
    if let Some(reference) = &operation.request {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.request.ref",
            &reference.reference,
            &[ReferenceKind::RequestContract],
        );
    }
    if let Some(reference) = &operation.decision_close {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.decision_close.ref",
            &reference.reference,
            &[ReferenceKind::DecisionCloseContract],
        );
    }
    if let Some(reference) = &operation.runtime_handoff {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.runtime_handoff.ref",
            &reference.reference,
            &[ReferenceKind::RuntimeHandoffContract],
        );
    }
    for gate in &operation.gates.required_before_mutation {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.gates.required_before_mutation.gate_contract_ref",
            &gate.gate_contract_ref,
            &[ReferenceKind::GateContract],
        );
    }
    for reference in &operation.gates.gate_contract_refs {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.gates.gate_contract_refs",
            reference,
            &[ReferenceKind::GateContract],
        );
    }
    for reference in &operation.effect_contract_refs {
        expect_repo_ref(
            &mut report,
            index,
            "operation_contract.effect_contract_refs",
            reference,
            &[ReferenceKind::ToolEffectContract],
        );
    }
    for command in &operation.command_refs {
        expect_stable_ref(
            &mut report,
            index,
            "operation_contract.command_refs.id",
            &command.id.0,
            &[ReferenceKind::CommandContract],
        );
    }

    report
}

#[must_use]
pub fn validate_claim_cross_references(
    claim: &ClaimContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let claim = &claim.claim_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "claim_contract.contract_ref",
        &claim.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &claim.claim.registry_ref {
        expect_repo_ref(
            &mut report,
            index,
            "claim_contract.claim.registry_ref",
            reference,
            &[
                ReferenceKind::RuntimeRegistryEntry,
                ReferenceKind::RuntimeRegistryProjection,
            ],
        );
    }
    if let Some(reference) = &claim.expiry_policy.handoff_request_ref {
        expect_repo_ref(
            &mut report,
            index,
            "claim_contract.expiry_policy.handoff_request_ref",
            reference,
            &[ReferenceKind::RequestContract],
        );
    }

    report
}

#[must_use]
pub fn validate_completion_cross_references(
    completion: &CompletionContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let completion = &completion.completion_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "completion_contract.contract_ref",
        &completion.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &completion.claim.claim_contract_ref {
        expect_repo_ref(
            &mut report,
            index,
            "completion_contract.claim.claim_contract_ref",
            reference,
            &[ReferenceKind::ClaimContract],
        );
    }
    if let Some(reference) = &completion.storage.event_log_ref {
        expect_repo_ref(
            &mut report,
            index,
            "completion_contract.storage.event_log_ref",
            reference,
            &[ReferenceKind::Ledger],
        );
    }

    report
}

#[must_use]
pub fn validate_gate_cross_references(
    gate: &GateContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let gate = &gate.gate_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "gate_contract.contract_ref",
        &gate.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &gate.promotion.parent_gate_ref {
        expect_repo_ref(
            &mut report,
            index,
            "gate_contract.promotion.parent_gate_ref",
            reference,
            &[ReferenceKind::GateContract],
        );
    }

    report
}

#[must_use]
pub fn validate_request_cross_references(
    request: &RequestContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let request = &request.request_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "request_contract.contract_ref",
        &request.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    for dependency in &request.payload.dependency_refs {
        let expected = match dependency.kind {
            forge_core_contracts::request::DependencyKind::Request => {
                ReferenceKind::RequestContract
            }
            forge_core_contracts::request::DependencyKind::Claim => ReferenceKind::ClaimContract,
            forge_core_contracts::request::DependencyKind::Gate => ReferenceKind::GateContract,
            forge_core_contracts::request::DependencyKind::Effect => {
                ReferenceKind::ToolEffectContract
            }
            forge_core_contracts::request::DependencyKind::RuntimeHandoff => {
                ReferenceKind::RuntimeHandoffContract
            }
            forge_core_contracts::request::DependencyKind::Decision => {
                ReferenceKind::DecisionCloseContract
            }
        };
        expect_stable_ref(
            &mut report,
            index,
            "request_contract.payload.dependency_refs.ref",
            &dependency.reference,
            &[expected],
        );
    }

    report
}

#[must_use]
pub fn validate_tool_effect_cross_references(
    effect: &ToolEffectContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let effect = &effect.tool_effect_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "tool_effect_contract.contract_ref",
        &effect.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &effect.notification.request_contract_ref {
        expect_repo_ref(
            &mut report,
            index,
            "tool_effect_contract.notification.request_contract_ref",
            reference,
            &[ReferenceKind::RequestContract],
        );
    }
    if let Some(reference) = &effect.repair.inverse_operation_ref {
        expect_stable_ref(
            &mut report,
            index,
            "tool_effect_contract.repair.inverse_operation_ref",
            reference,
            &[ReferenceKind::ToolEffectContract],
        );
    }
    if let Some(reference) = &effect.repair.inverse.reference {
        expect_stable_ref(
            &mut report,
            index,
            "tool_effect_contract.repair.inverse.ref",
            reference,
            &[ReferenceKind::ToolEffectContract],
        );
    }
    for reference in &effect.repair.inverse.validation_gate_refs {
        expect_repo_ref(
            &mut report,
            index,
            "tool_effect_contract.repair.inverse.validation_gate_refs",
            reference,
            &[ReferenceKind::GateContract],
        );
    }

    report
}

#[must_use]
pub fn validate_decision_close_cross_references(
    decision: &DecisionCloseContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let decision = &decision.decision_close_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "decision_close_contract.contract_ref",
        &decision.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &decision.grill_contract_ref {
        expect_repo_ref(
            &mut report,
            index,
            "decision_close_contract.grill_contract_ref",
            reference,
            &[ReferenceKind::GateContract],
        );
    }

    report
}

#[must_use]
pub fn validate_runtime_handoff_cross_references(
    handoff: &RuntimeHandoffContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let handoff = &handoff.runtime_handoff_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "runtime_handoff_contract.contract_ref",
        &handoff.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &handoff.target_runtime.registry_ref {
        expect_repo_ref(
            &mut report,
            index,
            "runtime_handoff_contract.target_runtime.registry_ref",
            reference,
            &[ReferenceKind::RuntimeRegistryEntry],
        );
    }
    if let Some(reference) = &handoff.double_gate.registry_evidence_ref {
        expect_repo_ref(
            &mut report,
            index,
            "runtime_handoff_contract.double_gate.registry_evidence_ref",
            reference,
            &[ReferenceKind::RuntimeRegistryEntry],
        );
    }
    if let Some(reference) = &handoff.double_gate.capability_evidence_ref {
        expect_repo_ref(
            &mut report,
            index,
            "runtime_handoff_contract.double_gate.capability_evidence_ref",
            reference,
            &[ReferenceKind::RuntimeCapability],
        );
    }
    for reference in &handoff.handoff_payload.context_refs {
        expect_repo_ref(
            &mut report,
            index,
            "runtime_handoff_contract.handoff_payload.context_refs",
            reference,
            authority_or_fixture_kinds(),
        );
    }
    for reference in &handoff.handoff_payload.authority_refs {
        expect_repo_ref(
            &mut report,
            index,
            "runtime_handoff_contract.handoff_payload.authority_refs",
            reference,
            authority_or_fixture_kinds(),
        );
    }

    report
}

#[must_use]
pub fn validate_runtime_registry_cross_references(
    registry: &RuntimeRegistryEntryDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let registry = &registry.runtime_registry_entry;
    let mut report = ValidationReport::new();

    for reference in &registry.capability_refs {
        expect_repo_ref(
            &mut report,
            index,
            "runtime_registry_entry.capability_refs",
            reference,
            &[ReferenceKind::RuntimeCapability],
        );
    }

    report
}

#[must_use]
pub fn validate_health_recovery_cross_references(
    recovery: &HealthRecoveryContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let recovery = &recovery.health_recovery_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "health_recovery_contract.contract_ref",
        &recovery.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    if let Some(reference) = &recovery.recovery.request_ref {
        expect_repo_ref(
            &mut report,
            index,
            "health_recovery_contract.recovery.request_ref",
            reference,
            &[ReferenceKind::RequestContract],
        );
    }
    if let Some(reference) = &recovery.recovery.claim_ref {
        expect_repo_ref(
            &mut report,
            index,
            "health_recovery_contract.recovery.claim_ref",
            reference,
            &[ReferenceKind::ClaimContract],
        );
    }
    for reference in &recovery.recovery.handoff_context_refs {
        expect_repo_ref(
            &mut report,
            index,
            "health_recovery_contract.recovery.handoff_context_refs",
            reference,
            authority_or_fixture_kinds(),
        );
    }

    report
}

#[must_use]
pub fn validate_coordination_eval_cross_references(
    eval: &CoordinationEvalContractDocument,
    index: &ReferenceIndex,
) -> ValidationReport {
    let eval = &eval.coordination_eval_contract;
    let mut report = ValidationReport::new();

    expect_repo_ref(
        &mut report,
        index,
        "coordination_eval_contract.contract_ref",
        &eval.contract_ref,
        &[ReferenceKind::ContractDefinition],
    );
    for dimension in &eval.dimensions {
        for reference in &dimension.fixture_refs {
            expect_repo_ref(
                &mut report,
                index,
                "coordination_eval_contract.dimensions.fixture_refs",
                reference,
                &[ReferenceKind::OperationFixture],
            );
        }
        for reference in &dimension.evidence_refs {
            expect_repo_ref(
                &mut report,
                index,
                "coordination_eval_contract.dimensions.evidence_refs",
                reference,
                authority_or_definition_kinds(),
            );
        }
    }

    report
}

#[instrument(skip_all, fields(command_id = %command.command_contract.id.0))]
pub fn validate_command(command: &CommandContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let command = &command.command_contract;

    if command.args.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::EmptyCommandArgs,
            format!("command_contract.{}", command.id.0),
            "command args must not be empty",
        ));
    }
    if command.safety.shell_string_allowed {
        report.push(Diagnostic::error(
            DiagnosticCode::UnsafeCommandShellString,
            format!("command_contract.{}.safety", command.id.0),
            "shell string execution cannot be command authority",
        ));
    }
    if command.safety.publishes
        && !matches!(
            command.side_effect_policy,
            forge_core_contracts::command::CommandSideEffectPolicy::Publish
        )
    {
        report.push(Diagnostic::error(
            DiagnosticCode::CommandPublishesWithoutPolicy,
            format!("command_contract.{}.side_effect_policy", command.id.0),
            "publishing command must declare publish side effect policy",
        ));
    }

    report
}

#[instrument(skip_all, fields(contract_id = %operation.operation_contract.contract_id.0))]
pub fn validate_operation(operation: &OperationContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let operation = &operation.operation_contract;

    if operation.stop_conditions.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::EmptyOperationStopConditions,
            format!("operation_contract.{}", operation.contract_id.0),
            "operation must declare stop conditions",
        ));
    }
    if operation.authority.authority_evidence.is_empty()
        && operation.authority.missing_authority.is_empty()
        && !matches!(
            operation.authority.mutation_policy,
            forge_core_contracts::operation::MutationPolicy::Forbidden
        )
    {
        report.push(Diagnostic::error(
            DiagnosticCode::MissingOperationAuthorityEvidence,
            format!("operation_contract.{}.authority", operation.contract_id.0),
            "non-forbidden operation requires authority evidence or explicit missing authority",
        ));
    }
    if matches!(
        operation.authority.mutation_policy,
        forge_core_contracts::operation::MutationPolicy::Allowed
    ) && matches!(
        operation.authority.side_effect_policy,
        forge_core_contracts::operation::OperationSideEffectPolicy::WriteProjectFiles
            | forge_core_contracts::operation::OperationSideEffectPolicy::RunCommands
            | forge_core_contracts::operation::OperationSideEffectPolicy::Publish
    ) && operation.effect_contract_refs.is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::MutatingOperationMissingEffectRefs,
            format!(
                "operation_contract.{}.effect_contract_refs",
                operation.contract_id.0
            ),
            "mutating operation must declare effect contract refs",
        ));
    }
    if operation.coordination_scope.concurrency.fleet_mode {
        if operation
            .coordination_scope
            .concurrency
            .agent_id
            .as_ref()
            .is_none_or(|value| value.0.trim().is_empty())
        {
            report.push(Diagnostic::error(
                DiagnosticCode::FleetOperationMissingAgent,
                format!(
                    "operation_contract.{}.concurrency.agent_id",
                    operation.contract_id.0
                ),
                "fleet operation requires agent_id",
            ));
        }
        if operation
            .coordination_scope
            .concurrency
            .registry_ref
            .as_ref()
            .is_none_or(|value| value.0.trim().is_empty())
        {
            report.push(Diagnostic::error(
                DiagnosticCode::FleetOperationMissingRegistry,
                format!(
                    "operation_contract.{}.concurrency.registry_ref",
                    operation.contract_id.0
                ),
                "fleet operation requires registry_ref",
            ));
        }
    }
    if matches!(
        operation.human.input_requirement,
        forge_core_contracts::operation::HumanInputRequirement::Required
    ) && operation.human.prompt.text.trim().is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::HumanPromptRequiredButEmpty,
            format!(
                "operation_contract.{}.human.prompt",
                operation.contract_id.0
            ),
            "required human input needs prompt text",
        ));
    }

    report
}

#[must_use]
pub fn validate_claim(claim: &ClaimContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let claim = &claim.claim_contract;

    if claim.lease.ttl_seconds == 0 {
        report.push(Diagnostic::error(
            DiagnosticCode::ClaimTtlMustBePositive,
            format!("claim_contract.{}.lease.ttl_seconds", claim.id.0),
            "claim ttl_seconds must be positive",
        ));
    }
    if claim.lease.heartbeat_interval_seconds == 0 {
        report.push(Diagnostic::error(
            DiagnosticCode::ClaimHeartbeatIntervalMustBePositive,
            format!(
                "claim_contract.{}.lease.heartbeat_interval_seconds",
                claim.id.0
            ),
            "claim heartbeat_interval_seconds must be positive",
        ));
    }
    if claim.expiry_policy.handoff_required
        && (claim.expiry_policy.release_without_handoff_allowed
            || claim.expiry_policy.handoff_request_ref.is_none())
    {
        report.push(Diagnostic::error(
            DiagnosticCode::ClaimHandoffPolicyInconsistent,
            format!("claim_contract.{}.expiry_policy", claim.id.0),
            "handoff-required claim needs handoff_request_ref and cannot allow release without handoff",
        ));
    }

    report
}

#[must_use]
pub fn validate_completion(completion: &CompletionContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let completion = &completion.completion_contract;

    if completion.proof_policy.required_for_done
        && completion.proof_policy.accepted_proof_kinds.is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::CompletionProofRequiredWithoutAcceptedKinds,
            format!("completion_contract.{}.proof_policy", completion.id.0),
            "completion requiring proof must list accepted proof kinds",
        ));
    }
    if matches!(
        completion.status.value,
        forge_core_contracts::completion::CompletionStatus::Done
    ) && completion.proof_policy.required_for_done
        && completion.proof_refs.is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::CompletionDoneMissingProof,
            format!("completion_contract.{}.proof_refs", completion.id.0),
            "done completion must include proof refs when proof is required",
        ));
    }

    report
}

#[must_use]
pub fn validate_gate(gate: &GateContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let gate = &gate.gate_contract;

    if matches!(
        gate.gate.status,
        forge_core_contracts::gate::GateStatus::Fail
            | forge_core_contracts::gate::GateStatus::Concerns
            | forge_core_contracts::gate::GateStatus::Missing
    ) && gate.blocks.operations.is_empty()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::GateBlockingStatusWithoutBlocks,
            format!("gate_contract.{}.blocks.operations", gate.id.0),
            "blocking gate status should declare blocked operations",
        ));
    }

    report
}

#[must_use]
pub fn validate_decision_close(decision: &DecisionCloseContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let decision = &decision.decision_close_contract;

    if decision.authority_evidence.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::DecisionAuthorityEvidenceMissing,
            format!(
                "decision_close_contract.{}.authority_evidence",
                decision.id.0
            ),
            "decision close requires authority evidence",
        ));
    }

    if matches!(decision.decision.kind, DecisionKind::PhaseTransition)
        && decision.decision.target_phase.is_none()
    {
        report.push(Diagnostic::error(
            DiagnosticCode::DecisionPhaseTransitionMissingTargetPhase,
            format!(
                "decision_close_contract.{}.decision.target_phase",
                decision.id.0
            ),
            "phase transition decision requires target_phase",
        ));
    }

    match decision.decision.status {
        DecisionStatus::Closed => {
            if decision.closed_at.is_none() {
                report.push(Diagnostic::error(
                    DiagnosticCode::DecisionClosedMissingTimestamp,
                    format!("decision_close_contract.{}.closed_at", decision.id.0),
                    "closed decision requires closed_at timestamp",
                ));
            }
            if decision.requires_grill_evidence {
                let has_grill_evidence = decision
                    .authority_evidence
                    .iter()
                    .any(|item| matches!(item.kind, DecisionEvidenceKind::GrillResult));
                if decision.grill_contract_ref.is_none() || !has_grill_evidence {
                    report.push(Diagnostic::error(
                        DiagnosticCode::DecisionClosedMissingGrillEvidence,
                        format!("decision_close_contract.{}.grill_contract_ref", decision.id.0),
                        "closed decision requiring grill needs grill_contract_ref and grill_result evidence",
                    ));
                }
            }
        }
        DecisionStatus::Blocked => {
            if decision.blocked_reason.is_none() {
                report.push(Diagnostic::error(
                    DiagnosticCode::DecisionBlockedMissingReason,
                    format!("decision_close_contract.{}.blocked_reason", decision.id.0),
                    "blocked decision requires blocked_reason",
                ));
            }
            if decision.closed_at.is_some() {
                report.push(Diagnostic::error(
                    DiagnosticCode::DecisionBlockedHasTimestamp,
                    format!("decision_close_contract.{}.closed_at", decision.id.0),
                    "blocked decision must not have closed_at timestamp",
                ));
            }
        }
        DecisionStatus::Open | DecisionStatus::Reopened => {}
    }

    report
}

#[must_use]
pub fn validate_runtime_handoff(handoff: &RuntimeHandoffContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let handoff = &handoff.runtime_handoff_contract;
    let registry_present = handoff.double_gate.registry_evidence_ref.is_some();
    let capability_present = handoff.double_gate.capability_evidence_ref.is_some();

    match handoff.status {
        RuntimeHandoffStatus::Blocked => {
            if handoff.blocked_reason.is_none() {
                report.push(Diagnostic::error(
                    DiagnosticCode::RuntimeHandoffBlockedMissingReason,
                    format!("runtime_handoff_contract.{}.blocked_reason", handoff.id.0),
                    "blocked runtime handoff requires blocked_reason",
                ));
            }
            if registry_present && capability_present {
                report.push(Diagnostic::error(
                    DiagnosticCode::RuntimeHandoffBlockedHasCompleteDoubleGate,
                    format!("runtime_handoff_contract.{}.double_gate", handoff.id.0),
                    "blocked runtime handoff should be missing at least one double-gate evidence ref",
                ));
            }
        }
        RuntimeHandoffStatus::Suggestible
        | RuntimeHandoffStatus::Requested
        | RuntimeHandoffStatus::Accepted
        | RuntimeHandoffStatus::Rejected => {
            if handoff.blocked_reason.is_some() {
                report.push(Diagnostic::error(
                    DiagnosticCode::RuntimeHandoffNonBlockedHasBlockedReason,
                    format!("runtime_handoff_contract.{}.blocked_reason", handoff.id.0),
                    "non-blocked runtime handoff must not carry blocked_reason",
                ));
            }
            if handoff.double_gate.requires_registry_evidence && !registry_present {
                report.push(Diagnostic::error(
                    DiagnosticCode::RuntimeHandoffMissingRegistryEvidence,
                    format!(
                        "runtime_handoff_contract.{}.double_gate.registry_evidence_ref",
                        handoff.id.0
                    ),
                    "runtime handoff requires registry evidence before suggestion or request",
                ));
            }
            if handoff.double_gate.requires_capability_evidence && !capability_present {
                report.push(Diagnostic::error(
                    DiagnosticCode::RuntimeHandoffMissingCapabilityEvidence,
                    format!(
                        "runtime_handoff_contract.{}.double_gate.capability_evidence_ref",
                        handoff.id.0
                    ),
                    "runtime handoff requires capability evidence before suggestion or request",
                ));
            }
        }
    }

    if handoff.handoff_payload.context_refs.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::RuntimeHandoffPayloadContextEmpty,
            format!(
                "runtime_handoff_contract.{}.handoff_payload.context_refs",
                handoff.id.0
            ),
            "runtime handoff payload should pass compact context refs",
        ));
    }
    if handoff.handoff_payload.authority_refs.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::RuntimeHandoffPayloadAuthorityEmpty,
            format!(
                "runtime_handoff_contract.{}.handoff_payload.authority_refs",
                handoff.id.0
            ),
            "runtime handoff payload must preserve Forge authority refs",
        ));
    }

    report
}

#[must_use]
pub fn validate_runtime_registry_entry(
    registry: &RuntimeRegistryEntryDocument,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    let registry = &registry.runtime_registry_entry;

    if registry.capability_refs.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::RuntimeRegistryCapabilityRefsEmpty,
            format!("runtime_registry_entry.{}.capability_refs", registry.id.0),
            "runtime registry entry should declare capability refs",
        ));
    }
    if registry.trust_policy.may_mutate_forge_state
        || !registry.trust_policy.must_use_requests_for_state_changes
    {
        report.push(Diagnostic::error(
            DiagnosticCode::RuntimeRegistryUnsafeStateMutationPolicy,
            format!("runtime_registry_entry.{}.trust_policy", registry.id.0),
            "external runtime should not mutate Forge state directly and must use requests for state changes",
        ));
    }

    report
}

#[must_use]
pub fn validate_runtime_capability(capability: &RuntimeCapabilityDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let capability = &capability.runtime_capability;

    if !capability.constraints.output_must_be_evidence_ref {
        report.push(Diagnostic::error(
            DiagnosticCode::RuntimeCapabilityOutputEvidenceMissing,
            format!("runtime_capability.{}.constraints", capability.id.0),
            "runtime capability output must be evidence-ref addressable",
        ));
    }

    report
}

#[must_use]
pub fn validate_health_recovery(recovery: &HealthRecoveryContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let recovery = &recovery.health_recovery_contract;

    if recovery.heartbeat.ttl_seconds == 0 {
        report.push(Diagnostic::error(
            DiagnosticCode::HealthRecoveryTtlMustBePositive,
            format!(
                "health_recovery_contract.{}.heartbeat.ttl_seconds",
                recovery.id.0
            ),
            "health recovery heartbeat ttl_seconds must be positive",
        ));
    }
    if recovery.anomaly.evidence_refs.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::HealthRecoveryAnomalyEvidenceMissing,
            format!(
                "health_recovery_contract.{}.anomaly.evidence_refs",
                recovery.id.0
            ),
            "health recovery anomaly requires evidence refs",
        ));
    }

    let unhealthy = !matches!(
        recovery.status,
        HealthStatus::Healthy | HealthStatus::Unknown
    );
    if unhealthy && matches!(recovery.recovery.action, RecoveryAction::None) {
        report.push(Diagnostic::error(
            DiagnosticCode::HealthRecoveryUnhealthyWithoutAction,
            format!("health_recovery_contract.{}.recovery.action", recovery.id.0),
            "unhealthy runtime state requires a recovery action",
        ));
    }
    if recovery.recovery.requires_review && recovery.recovery.automatic_allowed {
        report.push(Diagnostic::error(
            DiagnosticCode::HealthRecoveryReviewAllowsAutomaticAction,
            format!(
                "health_recovery_contract.{}.recovery.automatic_allowed",
                recovery.id.0
            ),
            "review-required recovery cannot also be automatic",
        ));
    }
    if recovery.recovery.requires_review
        && (recovery.recovery.request_ref.is_none() || recovery.recovery.claim_ref.is_none())
    {
        report.push(Diagnostic::error(
            DiagnosticCode::HealthRecoveryReviewMissingRefs,
            format!("health_recovery_contract.{}.recovery", recovery.id.0),
            "review-required recovery should reference request and claim contracts",
        ));
    }
    if recovery.recovery.requires_review && recovery.recovery.handoff_context_refs.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::HealthRecoveryHandoffContextMissing,
            format!(
                "health_recovery_contract.{}.recovery.handoff_context_refs",
                recovery.id.0
            ),
            "review-required recovery should preserve compact handoff context refs",
        ));
    }

    report
}

#[must_use]
pub fn validate_coordination_eval(eval: &CoordinationEvalContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let eval = &eval.coordination_eval_contract;

    if eval.dimensions.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::CoordinationEvalDimensionsEmpty,
            format!("coordination_eval_contract.{}.dimensions", eval.id.0),
            "coordination eval requires dimensions",
        ));
        return report;
    }

    let mut seen = HashSet::new();
    for dimension in &eval.dimensions {
        if !seen.insert(dimension.dimension) {
            report.push(Diagnostic::error(
                DiagnosticCode::CoordinationEvalDuplicateDimension,
                format!(
                    "coordination_eval_contract.{}.dimensions.{:?}",
                    eval.id.0, dimension.dimension
                ),
                "coordination eval dimension must be unique",
            ));
        }
        if dimension.fixture_refs.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::CoordinationEvalFixtureRefsEmpty,
                format!(
                    "coordination_eval_contract.{}.dimensions.{:?}.fixture_refs",
                    eval.id.0, dimension.dimension
                ),
                "coordination eval dimension requires fixture refs",
            ));
        }
        if dimension.evidence_refs.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::CoordinationEvalEvidenceRefsEmpty,
                format!(
                    "coordination_eval_contract.{}.dimensions.{:?}.evidence_refs",
                    eval.id.0, dimension.dimension
                ),
                "coordination eval dimension requires evidence refs",
            ));
        }
        if dimension.failure_signal.trim().is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::CoordinationEvalFailureSignalEmpty,
                format!(
                    "coordination_eval_contract.{}.dimensions.{:?}.failure_signal",
                    eval.id.0, dimension.dimension
                ),
                "coordination eval dimension requires concrete failure signal",
            ));
        }
        match dimension.metric_kind {
            CoordinationMetricKind::Threshold | CoordinationMetricKind::LatencyBudget => {
                if dimension.threshold.is_none() {
                    report.push(Diagnostic::error(
                        DiagnosticCode::CoordinationEvalThresholdMissing,
                        format!(
                            "coordination_eval_contract.{}.dimensions.{:?}.threshold",
                            eval.id.0, dimension.dimension
                        ),
                        "threshold or latency budget metric requires threshold",
                    ));
                }
            }
            CoordinationMetricKind::FixturePass | CoordinationMetricKind::ManualReview => {
                if dimension.threshold.is_some() {
                    report.push(Diagnostic::error(
                        DiagnosticCode::CoordinationEvalUnexpectedThreshold,
                        format!(
                            "coordination_eval_contract.{}.dimensions.{:?}.threshold",
                            eval.id.0, dimension.dimension
                        ),
                        "fixture_pass/manual_review metrics should not carry numeric threshold",
                    ));
                }
            }
        }
    }

    if eval.pass_policy.all_must_pass_dimensions_required {
        for required in CoordinationDimension::ALL {
            if !seen.contains(&required) {
                report.push(Diagnostic::error(
                    DiagnosticCode::CoordinationEvalMissingRequiredDimension,
                    format!("coordination_eval_contract.{}.dimensions", eval.id.0),
                    format!("coordination eval is missing required dimension {required:?}"),
                ));
            }
        }
    }
    if !eval.pass_policy.manual_review_blocks_release
        && eval.dimensions.iter().any(|dimension| {
            matches!(
                dimension.required_level,
                forge_core_contracts::coordination_eval::CoordinationRequiredLevel::ManualReviewRequired
            )
        })
    {
        report.push(Diagnostic::error(
            DiagnosticCode::CoordinationEvalManualReviewPolicyUnsafe,
            format!(
                "coordination_eval_contract.{}.pass_policy.manual_review_blocks_release",
                eval.id.0
            ),
            "manual-review-required dimensions must block release",
        ));
    }

    report
}

#[must_use]
pub fn validate_request(request: &RequestContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let request = &request.request_contract;

    if request.safety.may_mutate_integration_state {
        report.push(Diagnostic::error(
            DiagnosticCode::RequestMayMutateIntegrationState,
            format!("request_contract.{}.safety", request.id.0),
            "request append is not integration-state mutation authority",
        ));
    }
    if request.response.required != request.response_required {
        report.push(Diagnostic::error(
            DiagnosticCode::RequestResponseMismatch,
            format!("request_contract.{}.response", request.id.0),
            "canonical response.required must match compatibility response_required",
        ));
    }
    if request.append_only.path.0.trim().is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::RequestAppendOnlyPathMissing,
            format!("request_contract.{}.append_only.path", request.id.0),
            "request append-only stream path must not be empty",
        ));
    }

    report
}

#[must_use]
pub fn validate_tool_effect(effect: &ToolEffectContractDocument) -> ValidationReport {
    let mut report = ValidationReport::new();
    let effect = &effect.tool_effect_contract;

    if effect.read_set.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::EffectReadSetEmpty,
            format!("tool_effect_contract.{}.read_set", effect.id.0),
            "tool effect must declare read_set",
        ));
    }
    if effect.write_set.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::EffectWriteSetEmpty,
            format!("tool_effect_contract.{}.write_set", effect.id.0),
            "tool effect must declare write_set",
        ));
    }

    let has_destructive_write = effect.write_set.iter().any(|write| write.destructive);
    if has_destructive_write {
        let inverse = &effect.repair.inverse;
        let has_inverse = !matches!(
            inverse.kind,
            forge_core_contracts::tool_effect::InverseKind::None
        ) && !matches!(
            inverse.source,
            forge_core_contracts::tool_effect::InverseSource::Unavailable
        ) && inverse.reference.is_some();
        let stop_for_review = effect.repair.stop_if_inverse_missing
            && !effect.repair.automatic_repair_allowed
            && matches!(
                effect.conflict_detection.policy,
                forge_core_contracts::tool_effect::ConflictPolicy::HumanReview
                    | forge_core_contracts::tool_effect::ConflictPolicy::DriverReview
                    | forge_core_contracts::tool_effect::ConflictPolicy::Block
            );

        if !has_inverse && !stop_for_review {
            report.push(Diagnostic::error(
                DiagnosticCode::DestructiveEffectMissingInverseOrStop,
                format!("tool_effect_contract.{}.repair.inverse", effect.id.0),
                "destructive effect requires inverse metadata or explicit stop/review policy",
            ));
        }
        if has_inverse
            && effect.repair.automatic_repair_allowed
            && inverse.validation_gate_refs.is_empty()
        {
            report.push(Diagnostic::error(
                DiagnosticCode::DestructiveEffectInverseIncomplete,
                format!("tool_effect_contract.{}.repair.inverse", effect.id.0),
                "automatic inverse repair requires validation gate refs",
            ));
        }
    }

    report
}

fn expect_repo_ref(
    report: &mut ValidationReport,
    index: &ReferenceIndex,
    path: &str,
    reference: &RepoPath,
    expected: &[ReferenceKind],
) {
    expect_stable_ref(report, index, path, &reference.0, expected);
}

fn expect_stable_ref(
    report: &mut ValidationReport,
    index: &ReferenceIndex,
    path: &str,
    reference: &str,
    expected: &[ReferenceKind],
) {
    let Some(actual) = index.kind_of(reference) else {
        report.push(Diagnostic::error(
            DiagnosticCode::MissingReference,
            path,
            format!("missing reference {reference}"),
        ));
        return;
    };

    if !expected.contains(&actual) {
        report.push(Diagnostic::error(
            DiagnosticCode::ReferenceKindMismatch,
            path,
            format!("reference {reference} resolved as {actual:?}, expected one of {expected:?}"),
        ));
    }
}

fn authority_or_fixture_kinds() -> &'static [ReferenceKind] {
    &[
        ReferenceKind::OperationFixture,
        ReferenceKind::ClaimContract,
        ReferenceKind::CompletionContract,
        ReferenceKind::GateContract,
        ReferenceKind::RequestContract,
        ReferenceKind::ToolEffectContract,
        ReferenceKind::DecisionCloseContract,
        ReferenceKind::RuntimeHandoffContract,
        ReferenceKind::RuntimeRegistryEntry,
        ReferenceKind::RuntimeCapability,
        ReferenceKind::HealthRecoveryContract,
        ReferenceKind::CoordinationEvalContract,
        ReferenceKind::EvidenceArtifact,
    ]
}

fn authority_or_definition_kinds() -> &'static [ReferenceKind] {
    &[
        ReferenceKind::ContractDefinition,
        ReferenceKind::OperationFixture,
        ReferenceKind::ClaimContract,
        ReferenceKind::CompletionContract,
        ReferenceKind::GateContract,
        ReferenceKind::RequestContract,
        ReferenceKind::ToolEffectContract,
        ReferenceKind::DecisionCloseContract,
        ReferenceKind::RuntimeHandoffContract,
        ReferenceKind::RuntimeRegistryEntry,
        ReferenceKind::RuntimeCapability,
        ReferenceKind::HealthRecoveryContract,
        ReferenceKind::CoordinationEvalContract,
        ReferenceKind::EvidenceArtifact,
    ]
}

fn evidence_source_ids(registry: &FieldEvidenceRegistry) -> HashSet<&str> {
    registry
        .sources
        .iter()
        .map(|source| source.id.0.as_str())
        .collect()
}

fn check_source_ref(
    report: &mut ValidationReport,
    path: &str,
    source_id: &SourceId,
    known_sources: &HashSet<&str>,
) {
    if !known_sources.contains(source_id.0.as_str()) {
        report.push(Diagnostic::error(
            DiagnosticCode::UnknownEvidenceSourceRef,
            path,
            format!("unknown evidence source id {}", source_id.0),
        ));
    }
}
