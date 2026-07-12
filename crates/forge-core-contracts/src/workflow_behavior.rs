//! Closed contracts for P5d.3 behavioral shadow evidence.
//!
//! These documents bind reproducible inputs and normalized governed outcomes,
//! but deliberately cannot grant runtime or release authority. A successful
//! report is only a review candidate; later trusted admission remains a
//! separate concern.

use std::collections::BTreeSet;

use crate::assurance::{NextActionKind, ObligationStatus};
use crate::common::{RepoPath, StableId};
use crate::workflow_governance::WorkflowGovernanceEvaluationDocument;
use crate::workflow_release::{
    WorkflowGovernanceReleaseIdentity, WorkflowQuarantine, WorkflowRuntimeBundleIdentity,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BEHAVIORAL_SHADOW_REPORT_SCHEMA_VERSION: &str = "0.1";
pub const WORKFLOW_BEHAVIORAL_REVIEW_SUBJECT_SCHEMA_VERSION: &str = "0.1";

pub const WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_KIND: u16 = 1;
pub const WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_WORKFLOW: u16 = 7;
pub const WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS: u16 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralCoveragePolicyDocument {
    pub schema_version: String,
    pub workflow_behavioral_coverage_policy: WorkflowBehavioralCoveragePolicy,
}

/// Authored coverage settings are accepted only when they are at least as
/// strict as the closed P5d.3 floor. They never become admission authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
// These booleans are separate closed wire-level gates so an authored policy
// cannot collapse or ambiguously combine one mandatory coverage invariant.
#[allow(clippy::struct_excessive_bools)]
pub struct WorkflowBehavioralCoveragePolicy {
    pub id: StableId,
    pub policy_version: String,
    pub authority: WorkflowBehavioralEvidenceAuthority,
    pub required_scenario_kinds: Vec<WorkflowBehavioralScenarioKind>,
    pub minimum_scenarios_per_kind: u16,
    pub minimum_scenarios_per_workflow: u16,
    pub required_coverage_basis_points: u16,
    pub require_zero_mismatches: bool,
    pub require_zero_evaluation_errors: bool,
    pub required_dimensions: Vec<WorkflowGovernedOutcomeDimension>,
    pub require_resume_equivalence: bool,
    pub require_ablation_semantic_delta: bool,
    pub require_representative_scenarios: bool,
    pub require_adversarial_scenarios: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralEvidenceAuthority {
    NonAuthoritativeShadowEvidence,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralScenarioKind {
    Positive,
    Negative,
    Ambiguity,
    FalseCompletion,
    StaleEvidence,
    Resume,
    Ablation,
}

impl WorkflowBehavioralScenarioKind {
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::Positive,
            Self::Negative,
            Self::Ambiguity,
            Self::FalseCompletion,
            Self::StaleEvidence,
            Self::Resume,
            Self::Ablation,
        ]
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedOutcomeDimension {
    Status,
    Eligibility,
    Progression,
    Completion,
    Obligations,
    Claims,
    Decisions,
    Capabilities,
    Issues,
    NextActions,
}

impl WorkflowGovernedOutcomeDimension {
    #[must_use]
    pub const fn all() -> [Self; 10] {
        [
            Self::Status,
            Self::Eligibility,
            Self::Progression,
            Self::Completion,
            Self::Obligations,
            Self::Claims,
            Self::Decisions,
            Self::Capabilities,
            Self::Issues,
            Self::NextActions,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralScenarioCorpusDocument {
    pub schema_version: String,
    pub workflow_behavioral_scenario_corpus: WorkflowBehavioralScenarioCorpus,
}

/// Ordered content-addressed set of corpus partitions. Reports bind this
/// aggregate rather than pretending one representative or adversarial file is
/// the complete evidence universe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralCorpusSetDocument {
    pub schema_version: String,
    pub workflow_behavioral_corpus_set: WorkflowBehavioralCorpusSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralCorpusSet {
    pub id: StableId,
    pub corpus_set_version: String,
    pub authority: WorkflowBehavioralEvidenceAuthority,
    pub corpora: Vec<WorkflowBehavioralArtifactReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralScenarioCorpus {
    pub id: StableId,
    pub corpus_version: String,
    pub authority: WorkflowBehavioralEvidenceAuthority,
    pub partition_class: WorkflowBehavioralCorpusClass,
    pub coverage_policy: WorkflowBehavioralArtifactReference,
    pub workflow_evidence: Vec<WorkflowBehavioralWorkflowCorpus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralWorkflowCorpus {
    pub bindings: WorkflowBehavioralEvidenceBindings,
    pub scenarios: Vec<WorkflowBehavioralScenario>,
}

/// Exact identities required to prevent a passing report from being replayed
/// against another workflow, policy, batch, bundle, or release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralEvidenceBindings {
    /// Acyclic composition subject. It binds the proposed release identity,
    /// overlays, candidate bundle/policy set, workflow policies, quarantines,
    /// evaluator, and projection version while excluding evidence references,
    /// the final batch/manifest, and this report.
    pub review_subject: WorkflowBehavioralArtifactReference,
    /// Canonical JCS digest of the typed review subject.
    pub review_subject_digest: String,
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub policy_ref: StableId,
    pub policy_digest: String,
    pub candidate_bundle_id: StableId,
    /// Canonical JCS digest of the typed candidate bundle.
    pub candidate_bundle_digest: String,
    /// Exact sha256 digest of the embedded candidate bundle YAML bytes.
    pub candidate_bundle_source_digest: String,
    pub candidate_policy_set_digest: String,
    pub migration_batch_id: StableId,
    pub migration_batch_version: String,
    pub governance_release_id: StableId,
    pub governance_release_version: String,
    pub predecessor_release_digest: String,
    pub coverage_policy_id: StableId,
    /// Canonical JCS digest of the typed coverage policy.
    pub coverage_policy_digest: String,
    /// Exact sha256 digest of the embedded coverage policy YAML bytes.
    pub coverage_policy_source_digest: String,
    pub evaluator: WorkflowBehavioralEvaluatorIdentity,
    pub raw_sources: Vec<WorkflowBehavioralRawSourceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralEvaluatorIdentity {
    pub evaluator_id: StableId,
    pub evaluator_version: String,
    pub governed_projection_version: String,
    pub evaluator_source_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralRawSourceReference {
    pub kind: WorkflowBehavioralRawSourceKind,
    pub embedded_ref: RepoPath,
    pub expected_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralRawSourceKind {
    LegacyWorkflow,
    GovernancePolicy,
    CandidateBundle,
    CoveragePolicy,
    Evaluator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralArtifactReference {
    pub id: StableId,
    pub embedded_ref: RepoPath,
    pub expected_digest: String,
}

/// Acyclic, typed subject reviewed by every P5d.3 scenario. It deliberately
/// excludes evidence references, final batch/manifest digests, and reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralReviewSubjectDocument {
    pub schema_version: String,
    pub workflow_behavioral_review_subject: WorkflowBehavioralReviewSubject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralReviewSubject {
    pub id: StableId,
    pub authority: WorkflowBehavioralReviewSubjectAuthority,
    pub overlay: WorkflowBehavioralArtifactReference,
    /// Exact frozen admitted history against which replacement-agent
    /// continuity is audited while the candidate remains non-admitted.
    pub baseline_history: WorkflowBehavioralArtifactReference,
    pub baseline_release: WorkflowGovernanceReleaseIdentity,
    pub baseline_runtime_bundle: WorkflowRuntimeBundleIdentity,
    pub runtime_bundle: WorkflowBehavioralRuntimeBundleSubject,
    pub proposed_batch: WorkflowBehavioralProposedBatchSubject,
    pub proposed_release: WorkflowBehavioralProposedReleaseSubject,
    pub evaluator: WorkflowBehavioralEvaluatorIdentity,
    pub candidate_workflows: Vec<WorkflowBehavioralCandidateWorkflowSubject>,
    pub quarantines: Vec<WorkflowBehavioralQuarantineSubject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralReviewSubjectAuthority {
    CandidateOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralRuntimeBundleSubject {
    pub bundle_id: StableId,
    pub bundle_digest: String,
    pub policy_set_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralProposedBatchSubject {
    pub batch_id: StableId,
    pub batch_version: String,
    pub previous_batch_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralProposedReleaseSubject {
    pub lineage_id: StableId,
    pub release_id: StableId,
    pub release_version: String,
    pub previous_release_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralCandidateWorkflowSubject {
    pub workflow_id: StableId,
    pub legacy_workflow_digest: String,
    pub policy_ref: StableId,
    pub policy_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralQuarantineSubject {
    pub workflow_id: StableId,
    pub quarantine: WorkflowQuarantine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralScenario {
    pub scenario_id: StableId,
    pub scenario_kind: WorkflowBehavioralScenarioKind,
    pub corpus_class: WorkflowBehavioralCorpusClass,
    /// Digest of the canonical typed execution input with authored expected
    /// outcomes omitted. This avoids a corpus self-digest cycle and does not
    /// require one file per scenario.
    pub execution_input_digest: String,
    pub execution: WorkflowBehavioralScenarioExecution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralCorpusClass {
    Representative,
    Adversarial,
}

/// The bundle remains an external content-addressed artifact while the typed
/// evaluation input is embedded, allowing deterministic recomputation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralGovernanceInput {
    pub bundle: WorkflowBehavioralArtifactReference,
    pub evaluation: WorkflowGovernanceEvaluationDocument,
}

/// Durable identity that a resume scenario must preserve across replacement
/// agents. The behavioral evaluator verifies these bindings against both
/// governance inputs; real WAL byte compatibility is exercised separately by
/// the frozen kernel fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralContinuationIdentity {
    pub ledger_digest: String,
    pub ledger_head_digest: String,
    pub snapshot_digest: String,
    pub active_release_id: StableId,
    pub active_release_digest: String,
    pub runtime_bundle_id: StableId,
    pub runtime_bundle_digest: String,
    pub state_version: u64,
    pub current_phase: StableId,
    pub observed_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBehavioralScenarioExecution {
    Single {
        input: Box<WorkflowBehavioralGovernanceInput>,
        expected: Box<WorkflowGovernedOutcome>,
    },
    Resume {
        continuation: Box<WorkflowBehavioralContinuationIdentity>,
        checkpoint_source: WorkflowBehavioralArtifactReference,
        checkpoint_digest: String,
        checkpoint_input: Box<WorkflowBehavioralGovernanceInput>,
        checkpoint_expected: Box<WorkflowGovernedOutcome>,
        resumed_input: Box<WorkflowBehavioralGovernanceInput>,
        resumed_expected: Box<WorkflowGovernedOutcome>,
        equivalence_dimensions: Vec<WorkflowGovernedOutcomeDimension>,
    },
    Ablation {
        control_input: Box<WorkflowBehavioralGovernanceInput>,
        control_expected: Box<WorkflowGovernedOutcome>,
        ablated_input: Box<WorkflowBehavioralGovernanceInput>,
        ablated_expected: Box<WorkflowGovernedOutcome>,
        removed_semantic_refs: Vec<StableId>,
        required_difference_dimensions: Vec<WorkflowGovernedOutcomeDimension>,
    },
}

/// Description- and rationale-free projection of every authority-relevant
/// result dimension. Producers must sort all vectors before comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernedOutcome {
    pub status: WorkflowGovernedStatus,
    pub eligibility: WorkflowGovernedEligibility,
    pub progression: WorkflowGovernedProgression,
    pub completion: WorkflowGovernedCompletion,
    pub obligations: Vec<WorkflowGovernedObligation>,
    pub claims: Vec<WorkflowGovernedClaim>,
    pub decision_refs: Vec<StableId>,
    pub capability_refs: Vec<StableId>,
    pub issues: Vec<WorkflowGovernedIssue>,
    pub next_actions: Vec<WorkflowGovernedNextAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedStatus {
    Ineligible,
    Blocked,
    Active,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedEligibility {
    Eligible,
    Ineligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedProgression {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedCompletion {
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernedObligation {
    pub obligation_id: StableId,
    pub status: ObligationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernedClaim {
    pub claim_id: StableId,
    pub status: WorkflowGovernedClaimStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedClaimStatus {
    Unknown,
    Supported,
    Verified,
    Waived,
    Disproven,
    Contradictory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernedIssue {
    pub code: WorkflowGovernedIssueCode,
    pub path: String,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowGovernedIssueCode {
    UnsupportedSchemaVersion,
    BlankRequiredField,
    DuplicateIdentifier,
    DuplicateReference,
    DanglingReference,
    DependencyCycle,
    InvalidEvaluator,
    InvalidDecisionRule,
    InvalidPolicy,
    BundleMismatch,
    UnknownPolicy,
    InvalidPhase,
    EvidenceBindingMismatch,
    UnsupportedEvidenceKind,
    InsufficientEvidenceStrength,
    StaleEvidence,
    InconclusiveEvidence,
    ContradictoryEvidence,
    PhaseIneligible,
    MissingPrerequisite,
    UnknownApplicability,
    InvalidWaiver,
    ExpiredWaiver,
    InsufficientPrincipalDiversity,
    InventedCompletionClaim,
    LegacyProjectionMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowGovernedNextAction {
    pub id: StableId,
    pub kind: NextActionKind,
    pub addresses_claim_refs: Vec<StableId>,
    pub rank: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralShadowReportDocument {
    pub schema_version: String,
    pub workflow_behavioral_shadow_report: WorkflowBehavioralShadowReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralShadowReport {
    pub id: StableId,
    pub report_version: String,
    pub authority: WorkflowBehavioralEvidenceAuthority,
    pub corpus: WorkflowBehavioralArtifactReference,
    pub coverage_policy: WorkflowBehavioralArtifactReference,
    pub workflow_reports: Vec<WorkflowBehavioralWorkflowReport>,
    pub verdict: WorkflowBehavioralVerdict,
    pub disposition: WorkflowBehavioralDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralWorkflowReport {
    pub bindings: WorkflowBehavioralEvidenceBindings,
    pub total_scenarios: u16,
    pub scenario_kind_counts: Vec<WorkflowBehavioralScenarioKindCount>,
    pub representative_scenarios: u16,
    pub adversarial_scenarios: u16,
    pub coverage_basis_points: u16,
    pub mismatch_count: u16,
    pub evaluation_error_count: u16,
    pub scenario_results: Vec<WorkflowBehavioralScenarioResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralScenarioKindCount {
    pub scenario_kind: WorkflowBehavioralScenarioKind,
    pub count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralScenarioResult {
    pub scenario_id: StableId,
    pub scenario_kind: WorkflowBehavioralScenarioKind,
    pub corpus_class: WorkflowBehavioralCorpusClass,
    pub execution: WorkflowBehavioralExecutionResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowBehavioralExecutionResult {
    Single {
        comparison: Box<WorkflowBehavioralOutcomeComparison>,
    },
    Resume {
        checkpoint: Box<WorkflowBehavioralOutcomeComparison>,
        resumed: Box<WorkflowBehavioralOutcomeComparison>,
        equivalent: bool,
    },
    Ablation {
        control: Box<WorkflowBehavioralOutcomeComparison>,
        ablated: Box<WorkflowBehavioralOutcomeComparison>,
        semantic_delta: bool,
        differing_dimensions: Vec<WorkflowGovernedOutcomeDimension>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBehavioralOutcomeComparison {
    pub expected: WorkflowGovernedOutcome,
    pub actual: WorkflowGovernedOutcome,
    pub differing_dimensions: Vec<WorkflowGovernedOutcomeDimension>,
}

impl WorkflowBehavioralOutcomeComparison {
    #[must_use]
    pub fn matches(&self) -> bool {
        self.expected == self.actual && self.differing_dimensions.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralVerdict {
    BehaviorallyConsistentCandidate,
    InsufficientEvidence,
    MismatchDetected,
    InvalidBindings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBehavioralDisposition {
    ReviewCandidate,
    QuarantineRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowBehavioralContractIssue {
    pub path: String,
    pub message: String,
}

impl WorkflowBehavioralReviewSubjectDocument {
    #[must_use]
    // Keep the complete content-addressed subject validation in one auditable
    // boundary; splitting it would obscure candidate/quarantine cross-checks.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<WorkflowBehavioralContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_BEHAVIORAL_REVIEW_SUBJECT_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let subject = &self.workflow_behavioral_review_subject;
        require_nonblank(&mut issues, "review_subject.id", &subject.id.0);
        validate_artifact(&mut issues, "review_subject.overlay", &subject.overlay);
        validate_artifact(
            &mut issues,
            "review_subject.baseline_history",
            &subject.baseline_history,
        );
        for (path, value) in [
            (
                "review_subject.baseline_release.lineage_id",
                &subject.baseline_release.lineage_id.0,
            ),
            (
                "review_subject.baseline_release.release_id",
                &subject.baseline_release.release_id.0,
            ),
            (
                "review_subject.baseline_release.release_version",
                &subject.baseline_release.release_version,
            ),
            (
                "review_subject.baseline_runtime_bundle.bundle_id",
                &subject.baseline_runtime_bundle.bundle_id.0,
            ),
        ] {
            require_nonblank(&mut issues, path, value);
        }
        for (path, value) in [
            (
                "review_subject.baseline_release.release_digest",
                &subject.baseline_release.release_digest,
            ),
            (
                "review_subject.baseline_runtime_bundle.bundle_digest",
                &subject.baseline_runtime_bundle.bundle_digest,
            ),
            (
                "review_subject.baseline_runtime_bundle.policy_set_digest",
                &subject.baseline_runtime_bundle.policy_set_digest,
            ),
        ] {
            require_digest(&mut issues, path, value);
        }
        require_nonblank(
            &mut issues,
            "review_subject.runtime_bundle.bundle_id",
            &subject.runtime_bundle.bundle_id.0,
        );
        require_digest(
            &mut issues,
            "review_subject.runtime_bundle.bundle_digest",
            &subject.runtime_bundle.bundle_digest,
        );
        require_digest(
            &mut issues,
            "review_subject.runtime_bundle.policy_set_digest",
            &subject.runtime_bundle.policy_set_digest,
        );
        for (path, value) in [
            (
                "review_subject.proposed_batch.batch_id",
                &subject.proposed_batch.batch_id.0,
            ),
            (
                "review_subject.proposed_batch.batch_version",
                &subject.proposed_batch.batch_version,
            ),
            (
                "review_subject.proposed_release.lineage_id",
                &subject.proposed_release.lineage_id.0,
            ),
            (
                "review_subject.proposed_release.release_id",
                &subject.proposed_release.release_id.0,
            ),
            (
                "review_subject.proposed_release.release_version",
                &subject.proposed_release.release_version,
            ),
        ] {
            require_nonblank(&mut issues, path, value);
        }
        require_digest(
            &mut issues,
            "review_subject.proposed_batch.previous_batch_digest",
            &subject.proposed_batch.previous_batch_digest,
        );
        require_digest(
            &mut issues,
            "review_subject.proposed_release.previous_release_digest",
            &subject.proposed_release.previous_release_digest,
        );
        require_nonblank(
            &mut issues,
            "review_subject.evaluator.evaluator_id",
            &subject.evaluator.evaluator_id.0,
        );
        require_nonblank(
            &mut issues,
            "review_subject.evaluator.evaluator_version",
            &subject.evaluator.evaluator_version,
        );
        require_nonblank(
            &mut issues,
            "review_subject.evaluator.governed_projection_version",
            &subject.evaluator.governed_projection_version,
        );
        require_digest(
            &mut issues,
            "review_subject.evaluator.evaluator_source_digest",
            &subject.evaluator.evaluator_source_digest,
        );
        if subject.candidate_workflows.is_empty() {
            issue(
                &mut issues,
                "review_subject.candidate_workflows",
                "at least one candidate workflow is required",
            );
        }
        let mut candidates = BTreeSet::new();
        for (index, candidate) in subject.candidate_workflows.iter().enumerate() {
            let path = format!("review_subject.candidate_workflows[{index}]");
            for (field, value) in [
                ("workflow_id", &candidate.workflow_id.0),
                ("policy_ref", &candidate.policy_ref.0),
            ] {
                require_nonblank(&mut issues, &format!("{path}.{field}"), value);
            }
            require_digest(
                &mut issues,
                &format!("{path}.legacy_workflow_digest"),
                &candidate.legacy_workflow_digest,
            );
            require_digest(
                &mut issues,
                &format!("{path}.policy_digest"),
                &candidate.policy_digest,
            );
            if !candidates.insert(&candidate.workflow_id.0) {
                issue(
                    &mut issues,
                    &format!("{path}.workflow_id"),
                    "duplicate candidate workflow",
                );
            }
        }
        let mut quarantines = BTreeSet::new();
        for (index, quarantine) in subject.quarantines.iter().enumerate() {
            let path = format!("review_subject.quarantines[{index}].workflow_id");
            require_nonblank(&mut issues, &path, &quarantine.workflow_id.0);
            if !quarantines.insert(&quarantine.workflow_id.0) {
                issue(&mut issues, &path, "duplicate quarantined workflow");
            }
            if candidates.contains(&quarantine.workflow_id.0) {
                issue(
                    &mut issues,
                    &path,
                    "workflow cannot be both candidate and quarantined",
                );
            }
        }
        issues
    }
}

impl WorkflowBehavioralCoveragePolicyDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowBehavioralContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_BEHAVIORAL_COVERAGE_POLICY_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let policy = &self.workflow_behavioral_coverage_policy;
        require_nonblank(&mut issues, "policy.id", &policy.id.0);
        require_nonblank(&mut issues, "policy.policy_version", &policy.policy_version);
        require_exact_set(
            &mut issues,
            "policy.required_scenario_kinds",
            &policy.required_scenario_kinds,
            &WorkflowBehavioralScenarioKind::all(),
        );
        if policy.minimum_scenarios_per_kind < WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_KIND {
            issue(
                &mut issues,
                "policy.minimum_scenarios_per_kind",
                "coverage floor was weakened",
            );
        }
        if policy.minimum_scenarios_per_workflow
            < WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_WORKFLOW
        {
            issue(
                &mut issues,
                "policy.minimum_scenarios_per_workflow",
                "coverage floor was weakened",
            );
        }
        if policy.required_coverage_basis_points
            != WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS
        {
            issue(
                &mut issues,
                "policy.required_coverage_basis_points",
                "coverage must be exactly 10000 basis points",
            );
        }
        require_exact_set(
            &mut issues,
            "policy.required_dimensions",
            &policy.required_dimensions,
            &WorkflowGovernedOutcomeDimension::all(),
        );
        for (path, enabled) in [
            (
                "policy.require_zero_mismatches",
                policy.require_zero_mismatches,
            ),
            (
                "policy.require_zero_evaluation_errors",
                policy.require_zero_evaluation_errors,
            ),
            (
                "policy.require_resume_equivalence",
                policy.require_resume_equivalence,
            ),
            (
                "policy.require_ablation_semantic_delta",
                policy.require_ablation_semantic_delta,
            ),
            (
                "policy.require_representative_scenarios",
                policy.require_representative_scenarios,
            ),
            (
                "policy.require_adversarial_scenarios",
                policy.require_adversarial_scenarios,
            ),
        ] {
            if !enabled {
                issue(
                    &mut issues,
                    path,
                    "mandatory coverage gate cannot be disabled",
                );
            }
        }
        issues
    }
}

impl WorkflowBehavioralScenarioCorpusDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowBehavioralContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_BEHAVIORAL_SCENARIO_CORPUS_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let corpus = &self.workflow_behavioral_scenario_corpus;
        validate_artifact(
            &mut issues,
            "corpus.coverage_policy",
            &corpus.coverage_policy,
        );
        if corpus.workflow_evidence.is_empty() {
            issue(
                &mut issues,
                "corpus.workflow_evidence",
                "at least one workflow corpus is required",
            );
        }
        let mut workflow_ids = BTreeSet::new();
        for (index, workflow) in corpus.workflow_evidence.iter().enumerate() {
            let path = format!("corpus.workflow_evidence[{index}]");
            validate_bindings(&mut issues, &format!("{path}.bindings"), &workflow.bindings);
            if !workflow_ids.insert(&workflow.bindings.workflow_id.0) {
                issue(
                    &mut issues,
                    &format!("{path}.bindings.workflow_id"),
                    "duplicate workflow corpus",
                );
            }
            let mut scenario_ids = BTreeSet::new();
            for (scenario_index, scenario) in workflow.scenarios.iter().enumerate() {
                let scenario_path = format!("{path}.scenarios[{scenario_index}]");
                if !scenario_ids.insert(&scenario.scenario_id.0) {
                    issue(
                        &mut issues,
                        &format!("{scenario_path}.scenario_id"),
                        "duplicate scenario id",
                    );
                }
                if scenario.corpus_class != corpus.partition_class {
                    issue(
                        &mut issues,
                        &format!("{scenario_path}.corpus_class"),
                        "scenario class must match its corpus partition",
                    );
                }
                require_digest(
                    &mut issues,
                    &format!("{scenario_path}.execution_input_digest"),
                    &scenario.execution_input_digest,
                );
                validate_execution(
                    &mut issues,
                    &scenario_path,
                    scenario.scenario_kind,
                    &scenario.execution,
                    &workflow.bindings,
                );
            }
            if workflow.scenarios.is_empty() {
                issue(
                    &mut issues,
                    &format!("{path}.scenarios"),
                    "a workflow corpus partition cannot be empty",
                );
            }
            if corpus.coverage_policy.id != workflow.bindings.coverage_policy_id
                || corpus.coverage_policy.expected_digest
                    != workflow.bindings.coverage_policy_source_digest
            {
                issue(
                    &mut issues,
                    &format!("{path}.bindings.coverage_policy_digest"),
                    "workflow binding does not identify the corpus coverage policy",
                );
            }
        }
        issues
    }
}

impl WorkflowBehavioralCorpusSetDocument {
    #[must_use]
    pub fn validate(&self) -> Vec<WorkflowBehavioralContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_BEHAVIORAL_CORPUS_SET_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let set = &self.workflow_behavioral_corpus_set;
        require_nonblank(&mut issues, "corpus_set.id", &set.id.0);
        require_nonblank(
            &mut issues,
            "corpus_set.corpus_set_version",
            &set.corpus_set_version,
        );
        if set.corpora.is_empty() {
            issue(
                &mut issues,
                "corpus_set.corpora",
                "at least one corpus partition is required",
            );
        }
        let mut ids = BTreeSet::new();
        let mut refs = BTreeSet::new();
        for (index, corpus) in set.corpora.iter().enumerate() {
            validate_artifact(&mut issues, &format!("corpus_set.corpora[{index}]"), corpus);
            if !ids.insert(&corpus.id.0) || !refs.insert(&corpus.embedded_ref.0) {
                issue(
                    &mut issues,
                    &format!("corpus_set.corpora[{index}]"),
                    "duplicate corpus identity or path",
                );
            }
        }
        issues
    }
}

impl WorkflowBehavioralShadowReportDocument {
    #[must_use]
    // Aggregate hard gates are intentionally visible together so authored
    // report fields cannot bypass a check hidden in another validation pass.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Vec<WorkflowBehavioralContractIssue> {
        let mut issues = Vec::new();
        if self.schema_version != WORKFLOW_BEHAVIORAL_SHADOW_REPORT_SCHEMA_VERSION {
            issue(&mut issues, "schema_version", "unsupported schema version");
        }
        let report = &self.workflow_behavioral_shadow_report;
        validate_artifact(&mut issues, "report.corpus", &report.corpus);
        validate_artifact(
            &mut issues,
            "report.coverage_policy",
            &report.coverage_policy,
        );
        if report.workflow_reports.is_empty()
            && (report.verdict != WorkflowBehavioralVerdict::InvalidBindings
                || report.disposition != WorkflowBehavioralDisposition::QuarantineRequired)
        {
            issue(
                &mut issues,
                "report.workflow_reports",
                "an empty report is allowed only for invalid_bindings with quarantine_required",
            );
        }
        let mut consistent = !report.workflow_reports.is_empty();
        for (index, workflow) in report.workflow_reports.iter().enumerate() {
            let path = format!("report.workflow_reports[{index}]");
            let issues_before_bindings = issues.len();
            validate_bindings(&mut issues, &format!("{path}.bindings"), &workflow.bindings);
            if issues.len() != issues_before_bindings {
                consistent = false;
            }
            if report.coverage_policy.id != workflow.bindings.coverage_policy_id
                || report.coverage_policy.expected_digest
                    != workflow.bindings.coverage_policy_source_digest
            {
                issue(
                    &mut issues,
                    &format!("{path}.bindings.coverage_policy_digest"),
                    "workflow binding does not identify the report coverage policy",
                );
                consistent = false;
            }
            let counts = workflow
                .scenario_kind_counts
                .iter()
                .map(|entry| (entry.scenario_kind, entry.count))
                .collect::<Vec<_>>();
            let distinct = counts
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<BTreeSet<_>>();
            if distinct != WorkflowBehavioralScenarioKind::all().into_iter().collect()
                || counts.len() != 7
                || counts
                    .iter()
                    .any(|(_, count)| *count < WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_KIND)
            {
                issue(
                    &mut issues,
                    &format!("{path}.scenario_kind_counts"),
                    "every scenario kind requires a nonzero unique count",
                );
                consistent = false;
            }
            if workflow.total_scenarios < WORKFLOW_BEHAVIORAL_MINIMUM_SCENARIOS_PER_WORKFLOW
                || usize::from(workflow.total_scenarios) != workflow.scenario_results.len()
                || u32::from(workflow.total_scenarios)
                    != counts
                        .iter()
                        .map(|(_, count)| u32::from(*count))
                        .sum::<u32>()
            {
                issue(
                    &mut issues,
                    &format!("{path}.total_scenarios"),
                    "scenario totals are incomplete or inconsistent",
                );
                consistent = false;
            }
            if workflow.representative_scenarios == 0 || workflow.adversarial_scenarios == 0 {
                issue(
                    &mut issues,
                    &path,
                    "representative and adversarial coverage is required",
                );
                consistent = false;
            }
            let actual_representative = workflow
                .scenario_results
                .iter()
                .filter(|result| {
                    result.corpus_class == WorkflowBehavioralCorpusClass::Representative
                })
                .count();
            let actual_adversarial = workflow
                .scenario_results
                .iter()
                .filter(|result| result.corpus_class == WorkflowBehavioralCorpusClass::Adversarial)
                .count();
            if usize::from(workflow.representative_scenarios) != actual_representative
                || usize::from(workflow.adversarial_scenarios) != actual_adversarial
            {
                issue(
                    &mut issues,
                    &path,
                    "authored corpus-class totals do not match scenario results",
                );
                consistent = false;
            }
            if workflow.coverage_basis_points != WORKFLOW_BEHAVIORAL_REQUIRED_COVERAGE_BASIS_POINTS
                || workflow.mismatch_count != 0
                || workflow.evaluation_error_count != 0
            {
                issue(
                    &mut issues,
                    &path,
                    "full coverage with zero mismatches and errors is required",
                );
                consistent = false;
            }
            for (result_index, result) in workflow.scenario_results.iter().enumerate() {
                if !execution_result_matches(result.scenario_kind, &result.execution) {
                    issue(
                        &mut issues,
                        &format!("{path}.scenario_results[{result_index}]"),
                        "execution result is mismatched, non-equivalent, or lacks ablation delta",
                    );
                    consistent = false;
                }
            }
        }
        match (report.verdict, report.disposition) {
            (
                WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate,
                WorkflowBehavioralDisposition::ReviewCandidate,
            ) if consistent => {}
            (WorkflowBehavioralVerdict::BehaviorallyConsistentCandidate, _) => issue(
                &mut issues,
                "report.verdict",
                "consistent candidate requires every hard gate and review_candidate disposition",
            ),
            (_, WorkflowBehavioralDisposition::QuarantineRequired) => {}
            _ => issue(
                &mut issues,
                "report.disposition",
                "all non-consistent verdicts require quarantine",
            ),
        }
        issues
    }
}

// Resume and ablation are closed tagged variants whose complete validation is
// easier to audit as one exhaustive match than as loosely coupled helpers.
#[allow(clippy::too_many_lines)]
fn validate_execution(
    issues: &mut Vec<WorkflowBehavioralContractIssue>,
    path: &str,
    scenario_kind: WorkflowBehavioralScenarioKind,
    execution: &WorkflowBehavioralScenarioExecution,
    bindings: &WorkflowBehavioralEvidenceBindings,
) {
    match (scenario_kind, execution) {
        (
            WorkflowBehavioralScenarioKind::Resume,
            WorkflowBehavioralScenarioExecution::Resume {
                continuation,
                checkpoint_source,
                checkpoint_digest,
                checkpoint_input,
                resumed_input,
                equivalence_dimensions,
                ..
            },
        ) => {
            validate_artifact(
                issues,
                &format!("{path}.execution.checkpoint_source"),
                checkpoint_source,
            );
            for (field, value) in [
                ("ledger_digest", &continuation.ledger_digest),
                ("ledger_head_digest", &continuation.ledger_head_digest),
                ("snapshot_digest", &continuation.snapshot_digest),
                ("active_release_digest", &continuation.active_release_digest),
                ("runtime_bundle_digest", &continuation.runtime_bundle_digest),
            ] {
                require_digest(
                    issues,
                    &format!("{path}.execution.continuation.{field}"),
                    value,
                );
            }
            require_nonblank(
                issues,
                &format!("{path}.execution.continuation.active_release_id"),
                &continuation.active_release_id.0,
            );
            require_nonblank(
                issues,
                &format!("{path}.execution.continuation.runtime_bundle_id"),
                &continuation.runtime_bundle_id.0,
            );
            require_nonblank(
                issues,
                &format!("{path}.execution.continuation.current_phase"),
                &continuation.current_phase.0,
            );
            require_digest(
                issues,
                &format!("{path}.execution.checkpoint_digest"),
                checkpoint_digest,
            );
            validate_governance_input(
                issues,
                &format!("{path}.execution.checkpoint_input"),
                checkpoint_input,
            );
            validate_candidate_bundle_binding(
                issues,
                &format!("{path}.execution.checkpoint_input.bundle"),
                &checkpoint_input.bundle,
                bindings,
            );
            validate_governance_input(
                issues,
                &format!("{path}.execution.resumed_input"),
                resumed_input,
            );
            validate_candidate_bundle_binding(
                issues,
                &format!("{path}.execution.resumed_input.bundle"),
                &resumed_input.bundle,
                bindings,
            );
            let checkpoint = &checkpoint_input.evaluation.workflow_governance_evaluation;
            let resumed = &resumed_input.evaluation.workflow_governance_evaluation;
            if checkpoint != resumed {
                issue(
                    issues,
                    &format!("{path}.execution.continuation"),
                    "resume inputs must reconstruct the exact checkpoint evaluation",
                );
            }
            require_exact_set(
                issues,
                &format!("{path}.execution.equivalence_dimensions"),
                equivalence_dimensions,
                &WorkflowGovernedOutcomeDimension::all(),
            );
        }
        (
            WorkflowBehavioralScenarioKind::Ablation,
            WorkflowBehavioralScenarioExecution::Ablation {
                control_input,
                ablated_input,
                removed_semantic_refs,
                required_difference_dimensions,
                ..
            },
        ) => {
            validate_governance_input(
                issues,
                &format!("{path}.execution.control_input"),
                control_input,
            );
            validate_candidate_bundle_binding(
                issues,
                &format!("{path}.execution.control_input.bundle"),
                &control_input.bundle,
                bindings,
            );
            validate_governance_input(
                issues,
                &format!("{path}.execution.ablated_input"),
                ablated_input,
            );
            if removed_semantic_refs.is_empty() || required_difference_dimensions.is_empty() {
                issue(
                    issues,
                    &format!("{path}.execution"),
                    "ablation requires removed semantics and an expected governed delta",
                );
            }
            if control_input == ablated_input
                || control_input.bundle.expected_digest == ablated_input.bundle.expected_digest
            {
                issue(
                    issues,
                    &format!("{path}.execution.ablated_input"),
                    "ablation input and bundle digest must differ from the control",
                );
            }
        }
        (WorkflowBehavioralScenarioKind::Resume | WorkflowBehavioralScenarioKind::Ablation, _)
        | (
            _,
            WorkflowBehavioralScenarioExecution::Resume { .. }
            | WorkflowBehavioralScenarioExecution::Ablation { .. },
        ) => issue(
            issues,
            &format!("{path}.execution"),
            "scenario kind and execution shape do not match",
        ),
        (_, WorkflowBehavioralScenarioExecution::Single { input, .. }) => {
            validate_governance_input(issues, &format!("{path}.execution.input"), input);
            validate_candidate_bundle_binding(
                issues,
                &format!("{path}.execution.input.bundle"),
                &input.bundle,
                bindings,
            );
        }
    }
}

fn execution_result_matches(
    kind: WorkflowBehavioralScenarioKind,
    result: &WorkflowBehavioralExecutionResult,
) -> bool {
    match (kind, result) {
        (
            WorkflowBehavioralScenarioKind::Resume,
            WorkflowBehavioralExecutionResult::Resume {
                checkpoint,
                resumed,
                equivalent,
            },
        ) => checkpoint.matches() && resumed.matches() && *equivalent,
        (
            WorkflowBehavioralScenarioKind::Ablation,
            WorkflowBehavioralExecutionResult::Ablation {
                control,
                ablated,
                semantic_delta,
                differing_dimensions,
            },
        ) => {
            control.matches()
                && ablated.matches()
                && *semantic_delta
                && !differing_dimensions.is_empty()
        }
        (WorkflowBehavioralScenarioKind::Resume | WorkflowBehavioralScenarioKind::Ablation, _)
        | (
            _,
            WorkflowBehavioralExecutionResult::Resume { .. }
            | WorkflowBehavioralExecutionResult::Ablation { .. },
        ) => false,
        (_, WorkflowBehavioralExecutionResult::Single { comparison }) => comparison.matches(),
    }
}

fn validate_governance_input(
    issues: &mut Vec<WorkflowBehavioralContractIssue>,
    path: &str,
    input: &WorkflowBehavioralGovernanceInput,
) {
    validate_artifact(issues, &format!("{path}.bundle"), &input.bundle);
}

fn validate_candidate_bundle_binding(
    issues: &mut Vec<WorkflowBehavioralContractIssue>,
    path: &str,
    artifact: &WorkflowBehavioralArtifactReference,
    bindings: &WorkflowBehavioralEvidenceBindings,
) {
    if artifact.id != bindings.candidate_bundle_id
        || artifact.expected_digest != bindings.candidate_bundle_source_digest
    {
        issue(
            issues,
            path,
            "governance input is not bound to the candidate bundle",
        );
    }
}

// All identity and digest domains are checked together to make cross-domain
// reuse and missing raw-source bindings evident during review.
#[allow(clippy::too_many_lines)]
fn validate_bindings(
    issues: &mut Vec<WorkflowBehavioralContractIssue>,
    path: &str,
    binding: &WorkflowBehavioralEvidenceBindings,
) {
    validate_artifact(
        issues,
        &format!("{path}.review_subject"),
        &binding.review_subject,
    );
    for (name, value) in [
        ("workflow_id", binding.workflow_id.0.as_str()),
        ("policy_ref", binding.policy_ref.0.as_str()),
        (
            "candidate_bundle_id",
            binding.candidate_bundle_id.0.as_str(),
        ),
        ("migration_batch_id", binding.migration_batch_id.0.as_str()),
        (
            "migration_batch_version",
            binding.migration_batch_version.as_str(),
        ),
        (
            "governance_release_id",
            binding.governance_release_id.0.as_str(),
        ),
        (
            "governance_release_version",
            binding.governance_release_version.as_str(),
        ),
        ("coverage_policy_id", binding.coverage_policy_id.0.as_str()),
        (
            "evaluator.evaluator_id",
            binding.evaluator.evaluator_id.0.as_str(),
        ),
        (
            "evaluator.evaluator_version",
            binding.evaluator.evaluator_version.as_str(),
        ),
        (
            "evaluator.governed_projection_version",
            binding.evaluator.governed_projection_version.as_str(),
        ),
    ] {
        require_nonblank(issues, &format!("{path}.{name}"), value);
    }
    for (name, value) in [
        (
            "review_subject_digest",
            binding.review_subject_digest.as_str(),
        ),
        (
            "legacy_workflow_digest",
            binding.legacy_workflow_digest.as_str(),
        ),
        ("policy_digest", binding.policy_digest.as_str()),
        (
            "candidate_bundle_digest",
            binding.candidate_bundle_digest.as_str(),
        ),
        (
            "candidate_bundle_source_digest",
            binding.candidate_bundle_source_digest.as_str(),
        ),
        (
            "candidate_policy_set_digest",
            binding.candidate_policy_set_digest.as_str(),
        ),
        (
            "predecessor_release_digest",
            binding.predecessor_release_digest.as_str(),
        ),
        (
            "coverage_policy_digest",
            binding.coverage_policy_digest.as_str(),
        ),
        (
            "coverage_policy_source_digest",
            binding.coverage_policy_source_digest.as_str(),
        ),
        (
            "evaluator.evaluator_source_digest",
            binding.evaluator.evaluator_source_digest.as_str(),
        ),
    ] {
        require_digest(issues, &format!("{path}.{name}"), value);
    }
    let mut raw_sources = BTreeSet::new();
    for (index, source) in binding.raw_sources.iter().enumerate() {
        require_nonblank(
            issues,
            &format!("{path}.raw_sources[{index}].embedded_ref"),
            &source.embedded_ref.0,
        );
        require_digest(
            issues,
            &format!("{path}.raw_sources[{index}].expected_digest"),
            &source.expected_digest,
        );
        if !raw_sources.insert((&source.embedded_ref.0, &source.expected_digest)) {
            issue(
                issues,
                &format!("{path}.raw_sources[{index}]"),
                "duplicate raw source binding",
            );
        }
        if source.embedded_ref == binding.review_subject.embedded_ref {
            issue(
                issues,
                &format!("{path}.raw_sources[{index}]"),
                "review subject is already bound by review_subject and must not be duplicated",
            );
        }
    }
    if binding.raw_sources.is_empty() {
        issue(
            issues,
            &format!("{path}.raw_sources"),
            "raw content-addressed sources are required",
        );
    }
}

fn validate_artifact(
    issues: &mut Vec<WorkflowBehavioralContractIssue>,
    path: &str,
    artifact: &WorkflowBehavioralArtifactReference,
) {
    require_nonblank(issues, &format!("{path}.id"), &artifact.id.0);
    require_nonblank(
        issues,
        &format!("{path}.embedded_ref"),
        &artifact.embedded_ref.0,
    );
    require_digest(
        issues,
        &format!("{path}.expected_digest"),
        &artifact.expected_digest,
    );
}

fn require_digest(issues: &mut Vec<WorkflowBehavioralContractIssue>, path: &str, value: &str) {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !valid {
        issue(
            issues,
            path,
            "expected sha256 followed by 64 lowercase hexadecimal characters",
        );
    }
}

fn require_nonblank(issues: &mut Vec<WorkflowBehavioralContractIssue>, path: &str, value: &str) {
    if value.trim().is_empty() {
        issue(issues, path, "value must not be blank");
    }
}

fn require_exact_set<T: Copy + Ord>(
    issues: &mut Vec<WorkflowBehavioralContractIssue>,
    path: &str,
    actual: &[T],
    required: &[T],
) {
    let actual_set = actual.iter().copied().collect::<BTreeSet<_>>();
    let required_set = required.iter().copied().collect::<BTreeSet<_>>();
    if actual.len() != required.len() || actual_set != required_set {
        issue(
            issues,
            path,
            "the complete closed set is required exactly once",
        );
    }
}

fn issue(issues: &mut Vec<WorkflowBehavioralContractIssue>, path: &str, message: &str) {
    issues.push(WorkflowBehavioralContractIssue {
        path: path.to_owned(),
        message: message.to_owned(),
    });
}
