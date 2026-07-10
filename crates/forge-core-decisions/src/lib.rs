//! # forge-core-decisions
//!
//! A library of pure, deterministic decision functions for the Forge Method.
//! These functions take data in and return a verdict out — no IO, no mutable
//! state, and **no dependency on the mutation kernel**. The only crate-level
//! dependency is the typed [`forge_core_contracts`] layer (per ADR-0001's
//! deterministic Rust kernel). Mutation itself lives in `forge_core_kernel`;
//! this crate only *decides* what should be allowed to happen.
//!
//! ## What lives here
//!
//! - [`phase_transition`]: hard-gate enforcement — is a phase transition
//!   ALLOWED or BLOCKED, independent of what the host LLM suggests (DC1's
//!   "hard gate": the orchestrator reasons freely *within* gates; this module
//!   blocks illegal transitions).
//! - [`claim_engine`]: claims lifecycle and validity rules.
//! - [`isolation`]: worktree isolation decisions.
//! - [`autonomy_router`]: autonomy routing.
//! - [`catalog`]: workflow catalog selection and eligibility.
//! - [`coordination_eval`] / [`guide_validation`]: coordination and guide checks.
//! - [`execution_admission`]: P4a commit-time execution policy decision point.

// The coordination-eval aggregator and a few routers walk many independent
// dimensions while accumulating diagnostics; splitting them just to satisfy
// `clippy::too_many_lines` would scatter related checks.
#![allow(clippy::too_many_lines)]

pub mod autonomy_router;
pub mod catalog;
pub mod claim_engine;
pub mod conflict_detection;
pub mod coordination_eval;
pub mod embedded_contracts;
pub mod eval;
pub mod execution_admission;
pub mod guide_validation;
pub mod isolation;
pub mod obligation_engine;
pub mod phase_transition;

pub use catalog::{
    eligible_count, eligible_entries, find_entry, load_catalog, load_embedded_catalog,
    CatalogFileError, CatalogLoadReport,
};
pub use embedded_contracts::{
    embedded_exists, embedded_text, embedded_yaml_paths, read_contract_text,
};

pub use autonomy_router::{route_lane, LaneDecision, LaneKind, LaneRouteReason};
pub use eval::{
    load_eval_corpus, score_router, CaseScore, EvalCase, EvalCorpusDocument, RouterScore,
};
pub use execution_admission::{
    assurance_case_token, command_contract_token, effect_contract_token,
    evaluate_execution_admission, execution_intent_digest, operation_contract_token,
    ClaimRevisionObservation, ClaimSnapshotObservation, CommitAssuranceObservation,
    CompensationCoverage, ContentAddressedBinding, EffectContractBinding,
    ExecutionAdmissionDecision, ExecutionAdmissionInput, ExecutionAdmissionInputDocument,
    ExecutionAdmissionIssue, ExecutionAdmissionIssueCode, ExecutionAdmissionRejection,
    ExecutionAdmissionRequest, ExecutionAdmissionStatus, ExecutionCommitScope,
    ExecutionCommitStrategy, ExecutionPrincipalObservation, ExecutionPrincipalTrust,
    GateRevisionObservation, GateSnapshotObservation, GuaranteeStatus, ReplayProtectionObservation,
    ReplayReservationStatus, RevisionExpectation, SnapshotCompleteness,
    EXECUTION_ADMISSION_SCHEMA_VERSION, EXECUTION_AUTHORITY_SCOPE,
};
pub use guide_validation::{validate_guide_decision, GuideRejection, GuideValidation};

pub use phase_transition::{
    evaluate_transition, GateKind, ProvidedGateResult, TransitionBlockReason, TransitionDecision,
    TransitionRequest, Waiver,
};

pub use claim_engine::{
    acquire, claim_holds_scope, expire_stale, heartbeat, is_expired, is_live, project_active,
    reconcile_claims, record_handoff, release, rfc3339_to_unix, unix_to_rfc3339, AcquireRequest,
    ActiveClaimSummary, ActiveClaimsView, ClaimExpiry, ClaimLifecycleDecision,
    ClaimReconcileReport, ClaimReconcileTransition, ClaimRejection, RecordHandoffRequest,
};

pub use conflict_detection::{
    check_write_against_claims, repo_paths_overlap, BlockDetail, WriteCheck,
};
pub use isolation::{
    branch_name_for, detect_isolation_conflict, is_live as isolation_is_live, propose_merge,
    transition_status, validate_isolation_contract,
};
pub use obligation_engine::{
    derive_assurance_case, CapabilityAvailability, CapabilityObservation, DecisionNeed,
    EpistemicRiskSignal, LensApplicability, LensObservation, ObligationEngineInput,
    ObligationEngineInputDocument, ObligationEngineIssue, ObligationEngineRejection, RiskLevel,
    UniversalAssuranceLens, OBLIGATION_ENGINE_INPUT_SCHEMA_VERSION,
};

pub use coordination_eval::{
    coordination_fixture_gaps, score_coordination, validate_coordination_contract,
    CoordinationOutcome, CoordinationScore, CoordinationValidationError, CoordinationVerdict,
};
// Re-export the canonical phase type so downstream consumers can depend on the
// engine crate alone without reaching into contracts for the common case.
pub use forge_core_contracts::Phase;
