// The cross-reference and coordination-eval validators walk multiple
// contract fields and accumulate diagnostics per dimension. Splitting them
// to satisfy `clippy::too_many_lines` would scatter related checks across
// helpers and hurt audit readability.
#![allow(clippy::too_many_lines)]

pub mod assurance;
pub mod codes;
pub mod failure;
pub mod risk_audit;

pub use assurance::validate_assurance_case;

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

    /// Count of `Error`-severity diagnostics in this report. Generally useful
    /// for summary/stat counters; added during V2.B so the graph crate's
    /// `GraphValidationReport` (which carried its own `error_count`) could
    /// migrate onto the canonical report without losing the counter.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == DiagnosticSeverity::Error)
            .count()
    }

    /// Count of `Warning`-severity diagnostics in this report. Companion to
    /// [`error_count`](Self::error_count); added alongside it in V2.B.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == DiagnosticSeverity::Warning)
            .count()
    }

    /// Phase-gate: if this report carries any `Error`-severity diagnostics,
    /// return `Err(self)` so the caller can bail at a phase boundary — after
    /// accumulating *all* diagnostics in the phase. Modeled on rustc's
    /// `DiagCtxt::abort_if_errors`.
    ///
    /// Unlike short-circuiting on the first error, this lets the full phase run
    /// and collect every problem before deciding to stop: the caller runs a
    /// whole validation pass, accumulates into the report, then calls
    /// `report.abort_if_errors()?` to either continue to the next phase
    /// (`Ok(())`) or hand the *complete* report back to its caller (`Err(self)`).
    /// Nothing is dropped on the floor either way.
    ///
    /// Consumes `self` so the success path can drop the (empty or warning-only)
    /// report without the caller having to ignore it, and so the failure path
    /// moves ownership of the accumulated diagnostics to whoever handles the
    /// `Err`.
    ///
    /// # Errors
    ///
    /// Returns `Err(self)` when the report contains at least one
    /// `DiagnosticSeverity::Error` diagnostic; returns `Ok(())` otherwise.
    pub fn abort_if_errors(self) -> Result<(), Self> {
        if self.has_errors() {
            Err(self)
        } else {
            Ok(())
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    AssuranceUnsupportedSchemaVersion,
    AssuranceRequiredCollectionEmpty,
    AssuranceEmptyEntityId,
    AssuranceDuplicateEntityId,
    AssuranceDanglingClaimRef,
    AssuranceClaimEvidenceMissing,
    AssuranceWaiverInconsistent,
    AssuranceSatisfiedObligationUnsupported,
    AssuranceDecisionRecommendationInvalid,
    AssuranceCapabilityGapResolutionEmpty,
    AssuranceNextActionRankInvalid,
    AssuranceReadinessInconsistent,
    WorkflowGovernanceInvalid,
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
    // F07 — Multi-principal governance.
    GovernanceIntentAlreadyExpired,
    GovernanceIntentExpiryBeforeDeclared,
    GovernanceConflictPartiesNotDistinct,
    GovernanceConflictMissingParty,
    GovernanceConflictPolicySilentLastWriterWins,
    // F08 — Secure MCP adapter (allowlist + attestation).
    McpAllowlistYamlReadFailed,
    McpAllowlistYamlParseFailed,
    McpAllowlistUnknownTool,
    McpAllowlistDuplicateTool,
    McpAllowlistEmpty,
    McpAllowlistUnsafeReadOnlyPolicy,
    McpMutateGateMissingOperationContract,
    McpAttestationRequiredMissing,
    McpAttestationInvalid,
    // F14 — Knowledge Orchestration (citation check).
    UnresolvedSourceId,
    // V2.B — Workflow-graph diagnostics (migrated from forge-core-graph's
    // `GraphDiagnosticCode`). Each variant carries an explicit
    // `#[serde(rename)]` so the wire-format `code` string is byte-for-byte
    // identical to what the (now-deleted) `GraphDiagnosticCode` enum
    // serialized via `#[serde(rename_all = "snake_case")]` — graph/eval/harness
    // wire formats are regression anchors and MUST stay stable.
    #[serde(rename = "empty_graph_id")]
    GraphEmptyGraphId,
    #[serde(rename = "unsupported_schema_version")]
    GraphUnsupportedSchemaVersion,
    #[serde(rename = "invalid_graph_kind")]
    GraphInvalidGraphKind,
    #[serde(rename = "empty_graph")]
    GraphEmptyGraph,
    #[serde(rename = "empty_node_id")]
    GraphEmptyNodeId,
    #[serde(rename = "duplicate_node_id")]
    GraphDuplicateNodeId,
    #[serde(rename = "empty_edge_endpoint")]
    GraphEmptyEdgeEndpoint,
    #[serde(rename = "missing_edge_endpoint")]
    GraphMissingEdgeEndpoint,
    #[serde(rename = "cycle_detected")]
    GraphCycleDetected,
    #[serde(rename = "empty_operation_ref")]
    GraphEmptyOperationRef,
    #[serde(rename = "missing_operation_contract")]
    GraphMissingOperationContract,
    #[serde(rename = "invalid_operation_contract")]
    GraphInvalidOperationContract,
    #[serde(rename = "duplicate_operation_evaluation")]
    GraphDuplicateOperationEvaluation,
    #[serde(rename = "operation_not_ready")]
    GraphOperationNotReady,
    #[serde(rename = "operation_mutation_declaration_mismatch")]
    GraphOperationMutationDeclarationMismatch,
    #[serde(rename = "claim_preflight_blocked")]
    GraphClaimPreflightBlocked,
    #[serde(rename = "dangling_verifies_ref")]
    GraphDanglingVerifiesRef,
    #[serde(rename = "dangling_budget_node_ref")]
    GraphDanglingBudgetNodeRef,
    #[serde(rename = "edge_kind_source_kind_mismatch")]
    GraphEdgeKindSourceKindMismatch,
    // V2.B — Eval-comparison diagnostics (migrated from forge-core-eval's
    // `EvalDiagnosticCode`). Explicit `rename` preserves the original wire
    // strings (e.g. `missing_evidence_file`), which `eval_cli_e2e.rs` asserts on.
    #[serde(rename = "baseline_label_mismatch")]
    EvalBaselineLabelMismatch,
    #[serde(rename = "candidate_label_mismatch")]
    EvalCandidateLabelMismatch,
    #[serde(rename = "empty_run_set")]
    EvalEmptyRunSet,
    #[serde(rename = "task_count_below_minimum")]
    EvalTaskCountBelowMinimum,
    #[serde(rename = "task_set_mismatch")]
    EvalTaskSetMismatch,
    #[serde(rename = "missing_evidence_refs")]
    EvalMissingEvidenceRefs,
    #[serde(rename = "missing_trace_refs")]
    EvalMissingTraceRefs,
    #[serde(rename = "invalid_evidence_ref")]
    EvalInvalidEvidenceRef,
    #[serde(rename = "missing_evidence_file")]
    EvalMissingEvidenceFile,
    #[serde(rename = "evidence_ref_not_file")]
    EvalEvidenceRefNotFile,
    #[serde(rename = "evidence_ref_escapes_project")]
    EvalEvidenceRefEscapesProject,
    #[serde(rename = "duplicate_task_run")]
    EvalDuplicateTaskRun,
    #[serde(rename = "unsupported_run_schema_version")]
    EvalUnsupportedRunSchemaVersion,
    // V2.B — Eval-harness config diagnostics (migrated from
    // forge-core-eval-harness's `HarnessDiagnosticCode`).
    #[serde(rename = "unsupported_harness_schema_version")]
    HarnessUnsupportedSchemaVersion,
    #[serde(rename = "empty_arms")]
    HarnessEmptyArms,
    #[serde(rename = "fewer_than_two_arms")]
    HarnessFewerThanTwoArms,
    #[serde(rename = "duplicate_arm_label")]
    HarnessDuplicateArmLabel,
    #[serde(rename = "arm_command_empty")]
    HarnessArmCommandEmpty,
    #[serde(rename = "arm_timeout_zero")]
    HarnessArmTimeoutZero,
    #[serde(rename = "empty_corpus_ref")]
    HarnessEmptyCorpusRef,
    #[serde(rename = "empty_run_dir")]
    HarnessEmptyRunDir,
    #[serde(rename = "placeholder_missing")]
    HarnessPlaceholderMissing,
    // V2.B — Workspace-walk I/O diagnostics (migrated from
    // forge-core-cli/validate.rs's ad-hoc `String` code literals). These cover
    // filesystem/reference-index failures surfaced while walking a Forge
    // workspace; previously these were untyped strings. The `rename` matches
    // the exact literals validate.rs emitted before (e.g. `read_file_failed`).
    #[serde(rename = "read_file_failed")]
    ReadFileFailed,
    #[serde(rename = "parse_yaml_failed")]
    ParseYamlFailed,
    #[serde(rename = "read_dir_failed")]
    ReadDirFailed,
    #[serde(rename = "read_dir_entry_failed")]
    ReadDirEntryFailed,
    #[serde(rename = "reference_index_build_failed")]
    ReferenceIndexBuildFailed,
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

/// The F14 Citation Check (ADR-0010). Validates every `source_id` occurrence
/// in the parsed YAML documents against the **joint backing**: the curated
/// [`FieldEvidenceRegistry`] ∪ the runtime Source Ledger ids (`runtime_ids`).
/// A `source_id` that resolves in neither is an unresolved citation —
/// fail-closed, [`DiagnosticCode::UnresolvedSourceId`].
///
/// This is the F14 analog of [`validate_yaml_source_id_references`] (which
/// attests to resolution against the curated registry only, with
/// [`DiagnosticCode::UnknownEvidenceSourceRef`]). The citation check is broader:
/// it treats curated and runtime sources as one id namespace (ADR-0010 §1 —
/// the shared reuse boundary) and rejects any id that is registered nowhere.
///
/// `runtime_ids` is the set of *live* Source Ledger ids — i.e. the keys of a
/// `ResearchProjection::sources`, NOT the retired/superseded set. The caller
/// (CLI `research check`, runtime gate) builds this from the projection; the
/// validate crate stays decoupled from `forge-core-research` (which depends, via
/// `forge-core-store`, back on this crate — importing it here would form a
/// cycle). Passing data, not a projection type, is the same seam
/// [`validate_yaml_known_repo_references`] uses (`&HashSet<String>`).
///
/// In the MVP the check attests only to **resolution**, not to quality/tier
/// (ADR-0010 §5) — mirroring the admission gate, which also never attests to
/// truthfulness. A retired runtime source does NOT resolve: a citation to a
/// retired source is stale and must be re-issued.
///
/// Accumulating diagnostics per the repo convention (no short-circuit); the
/// caller decides via `report.has_errors()`.
// Mirrors `validate_yaml_known_repo_references`: the default `RandomState` keeps
// the API simple and matches the existing seam; downstream callers do not need
// to inject a custom hasher.
#[allow(clippy::implicit_hasher)]
#[must_use]
pub fn validate_yaml_citation_references(
    documents: &[ParsedYamlDocument],
    evidence: &FieldEvidenceRegistry,
    runtime_ids: &HashSet<String>,
) -> ValidationReport {
    // The joint id namespace: curated ∪ live-runtime.
    let mut resolvable: HashSet<&str> = evidence_source_ids(evidence);
    for id in runtime_ids {
        resolvable.insert(id.as_str());
    }

    let mut report = ValidationReport::new();
    for document in documents {
        validate_citation_ids_in_value(&mut report, &document.path, &document.value, &resolvable);
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

/// Walk a YAML value and report every `source_id` that does not resolve against
/// the joint curated ∪ runtime id namespace as
/// [`DiagnosticCode::UnresolvedSourceId`]. Structurally identical to
/// [`validate_source_ids_in_value`] but uses the citation-specific code so the
/// two populations of diagnostic stay distinguishable (a curated-only miss is
/// `UnknownEvidenceSourceRef`; a joint miss is `UnresolvedSourceId`).
fn validate_citation_ids_in_value(
    report: &mut ValidationReport,
    document_path: &str,
    value: &Value,
    resolvable: &HashSet<&str>,
) {
    match value {
        Value::Mapping(mapping) => {
            for (key, child) in mapping {
                if key.as_str() == Some("source_id") {
                    match child.as_str() {
                        Some(source_id) if resolvable.contains(source_id) => {}
                        Some(source_id) => report.push(Diagnostic::error(
                            DiagnosticCode::UnresolvedSourceId,
                            document_path,
                            format!("unresolved source_id {source_id} — not in field evidence registry or source ledger"),
                        )),
                        None => report.push(Diagnostic::error(
                            DiagnosticCode::SourceIdMustBeString,
                            document_path,
                            "source_id must be string",
                        )),
                    }
                }
                validate_citation_ids_in_value(report, document_path, child, resolvable);
            }
        }
        Value::Sequence(items) => {
            for item in items {
                validate_citation_ids_in_value(report, document_path, item, resolvable);
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

// --- F07 — Multi-principal governance validators ------------------------------
//
// Accumulating diagnostics per the repo convention (do not short-circuit on the
// first error). Each check pushes a typed Diagnostic; the caller decides via
// `report.has_errors()`.

/// Validate an [`forge_core_contracts::IntentContract`] (F07). Checks the load-bearing expiry
/// invariant: `expires_at` must be strictly greater than `declared_at` (an
/// already-expired intent is a deadlock/liveness footgun — Gray 2PL, Spanner).
#[must_use]
pub fn validate_intent_contract(intent: &forge_core_contracts::IntentContract) -> ValidationReport {
    let mut report = ValidationReport::new();
    let path = format!("intent.{}", intent.intent_id.0);

    if intent.expires_at <= intent.declared_at {
        report.push(Diagnostic::error(
            DiagnosticCode::GovernanceIntentExpiryBeforeDeclared,
            path.clone(),
            format!(
                "intent expires_at ({}) must be strictly greater than declared_at ({}) — an expiry at or before declaration is a permanent lock (deadlock/liveness failure)",
                intent.expires_at, intent.declared_at
            ),
        ));
    }

    if intent.principal.0.trim().is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::GovernanceConflictMissingParty,
            path,
            "intent principal must be a non-empty PrincipalId",
        ));
    }

    report
}

/// Validate a [`forge_core_contracts::ConflictContract`] (F07). The two parties must be present and
/// distinct (a principal cannot conflict with itself — that is a
/// self-overlap, a different concern). The contested scope target must be
/// non-empty.
#[must_use]
pub fn validate_conflict_contract(
    conflict: &forge_core_contracts::ConflictContract,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    let path = format!("conflict.{}", conflict.conflict_id.0);

    if conflict.principal_a.0.trim().is_empty() || conflict.principal_b.0.trim().is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::GovernanceConflictMissingParty,
            path.clone(),
            "conflict parties (principal_a, principal_b) must both be non-empty PrincipalIds",
        ));
    }
    if conflict.principal_a == conflict.principal_b {
        report.push(Diagnostic::error(
            DiagnosticCode::GovernanceConflictPartiesNotDistinct,
            path.clone(),
            "conflict parties must be distinct principals — a principal cannot conflict with itself",
        ));
    }
    if conflict.contested_scope.target.0.trim().is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::GovernanceConflictMissingParty,
            path,
            "conflict contested_scope.target must be a non-empty resource id",
        ));
    }

    report
}

/// Validate a [`forge_core_contracts::GovernancePolicy`] (F07). Warns (does not error) on
/// `SilentLastWriterWins` — it is the documented anti-pattern F07 exists to
/// forbid, but it is structurally permitted for completeness (a deployment
/// that opts into silent merge should do so loudly, with a warning on record).
#[must_use]
pub fn validate_governance_policy(
    policy: &forge_core_contracts::GovernancePolicy,
) -> ValidationReport {
    let mut report = ValidationReport::new();
    let path = format!("governance_policy.{}", policy.policy_id.0);

    if matches!(
        policy.conflict_policy,
        forge_core_contracts::ConflictPolicy::SilentLastWriterWins
    ) {
        report.push(Diagnostic::warning(
            DiagnosticCode::GovernanceConflictPolicySilentLastWriterWins,
            path,
            "conflict_policy is silent_last_writer_wins — this destroys the conflict signal (CRDT/XACML posture). F07's purpose is structured conflict (emit_contract); enable this only with explicit intent.",
        ));
    }

    report
}

#[cfg(test)]
mod governance_tests {
    use super::*;
    use forge_core_contracts::{
        ConflictContract, ConflictDetectionReason, ConflictPolicy, ConflictResolutionState,
        GovernancePolicy, IntentContract, IntentScope, IntentScopeKind, PrincipalId, StableId,
    };

    fn intent(id: &str, principal: &str, expires_at: u64, declared_at: u64) -> IntentContract {
        IntentContract {
            intent_id: StableId(id.into()),
            principal: PrincipalId(principal.into()),
            goal: "ship F07".into(),
            authority_scope: IntentScope {
                kind: IntentScopeKind::PathPrefix,
                target: StableId("contracts/stories".into()),
            },
            expires_at,
            declared_at,
        }
    }

    #[test]
    fn valid_intent_has_no_diagnostics() {
        let report = validate_intent_contract(&intent("i.1", "alice", 200, 100));
        assert!(!report.has_errors(), "{:?}", report.diagnostics());
        assert!(report.diagnostics().is_empty());
    }

    #[test]
    fn intent_with_expiry_at_or_before_declared_is_rejected() {
        // expiry == declared.
        let report = validate_intent_contract(&intent("i.1", "alice", 100, 100));
        assert!(report.has_errors());
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::GovernanceIntentExpiryBeforeDeclared));
        // expiry < declared.
        let report = validate_intent_contract(&intent("i.1", "alice", 50, 100));
        assert!(report.has_errors());
    }

    #[test]
    fn conflict_with_distinct_parties_is_valid() {
        let conflict = ConflictContract {
            conflict_id: StableId("c.1".into()),
            intent_a: StableId("i.1".into()),
            intent_b: StableId("i.2".into()),
            principal_a: PrincipalId("alice".into()),
            principal_b: PrincipalId("bob".into()),
            contested_scope: IntentScope {
                kind: IntentScopeKind::PathPrefix,
                target: StableId("contracts/stories".into()),
            },
            detection_reason: ConflictDetectionReason::PathOverlap,
            detected_at: 150,
            resolution: ConflictResolutionState::Pending,
        };
        let report = validate_conflict_contract(&conflict);
        assert!(!report.has_errors(), "{:?}", report.diagnostics());
    }

    #[test]
    fn conflict_with_identical_parties_is_rejected() {
        let conflict = ConflictContract {
            conflict_id: StableId("c.self".into()),
            intent_a: StableId("i.1".into()),
            intent_b: StableId("i.2".into()),
            principal_a: PrincipalId("alice".into()),
            principal_b: PrincipalId("alice".into()),
            contested_scope: IntentScope {
                kind: IntentScopeKind::PathPrefix,
                target: StableId("contracts/stories".into()),
            },
            detection_reason: ConflictDetectionReason::PathOverlap,
            detected_at: 150,
            resolution: ConflictResolutionState::Pending,
        };
        let report = validate_conflict_contract(&conflict);
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::GovernanceConflictPartiesNotDistinct));
    }

    #[test]
    fn governance_policy_warns_on_silent_last_writer_wins() {
        let policy = GovernancePolicy {
            policy_id: StableId("g.1".into()),
            permitted_principals: vec![PrincipalId("alice".into())],
            authorized_reviewers: vec![PrincipalId("daniel".into())],
            conflict_policy: ConflictPolicy::SilentLastWriterWins,
        };
        let report = validate_governance_policy(&policy);
        // Warning, NOT error — the posture is permitted but flagged.
        assert!(!report.has_errors());
        assert!(report.diagnostics().iter().any(|d| {
            d.code == DiagnosticCode::GovernanceConflictPolicySilentLastWriterWins
                && d.severity == DiagnosticSeverity::Warning
        }));
    }

    #[test]
    fn governance_policy_emit_contract_is_clean() {
        let policy = GovernancePolicy {
            policy_id: StableId("g.1".into()),
            permitted_principals: vec![PrincipalId("alice".into())],
            authorized_reviewers: vec![PrincipalId("daniel".into())],
            conflict_policy: ConflictPolicy::EmitContract,
        };
        let report = validate_governance_policy(&policy);
        assert!(report.diagnostics().is_empty());
    }
}
