//! # forge-core-engine
//!
//! The Forge Method surface: the state machine, phase transitions, and gate
//! enforcement. This crate owns the METHOD logic; it depends on the typed
//! contracts ([`forge_core_contracts`]) and sits above the runtime executor
//! (see the S1.1 engine-boundary decision).
//!
//! ## What lives here (slice 1)
//!
//! - [`phase_transition`]: hard-gate enforcement. The engine decides whether a
//!   phase transition is ALLOWED or BLOCKED, independent of what the host LLM
//!   suggests. This is the "hard gate" half of DC1: the orchestrator reasons
//!   freely *within* gates; the engine blocks illegal transitions.
//!
//! ## Hard gates (DC6)
//!
//! The `Specification -> Plan` transition mandates a passing **system-design**
//! gate. Even before the full system-design workflow exists, the engine
//! enforces the gate reference (content is filled in slice 3).

pub mod autonomy_router;
pub mod catalog;
pub mod claim_engine;
pub mod conflict_detection;
pub mod coordination_eval;
pub mod eval;
pub mod guide_validation;
pub mod isolation;
pub mod phase_transition;

pub use catalog::{
    eligible_count, eligible_entries, find_entry, load_catalog, load_embedded_catalog,
    CatalogFileError, CatalogLoadReport,
};

pub use autonomy_router::{route_lane, LaneDecision, LaneKind, LaneRouteReason};
pub use eval::{
    load_eval_corpus, score_router, CaseScore, EvalCase, EvalCorpusDocument, RouterScore,
};
pub use guide_validation::{validate_guide_decision, GuideRejection, GuideValidation};

pub use phase_transition::{
    evaluate_transition, GateKind, ProvidedGateResult, TransitionBlockReason, TransitionDecision,
    TransitionRequest, Waiver,
};

pub use claim_engine::{
    acquire, claim_holds_scope, expire_stale, heartbeat, is_expired, is_live, project_active,
    release, rfc3339_to_unix, unix_to_rfc3339, AcquireRequest, ActiveClaimSummary,
    ActiveClaimsView, ClaimExpiry, ClaimLifecycleDecision, ClaimRejection,
};

pub use conflict_detection::{
    check_write_against_claims, repo_paths_overlap, BlockDetail, WriteCheck,
};
pub use isolation::{
    branch_name_for, detect_isolation_conflict, is_live as isolation_is_live, propose_merge,
    transition_status, validate_isolation_contract,
};

pub use coordination_eval::{
    coordination_fixture_gaps, score_coordination, validate_coordination_contract,
    CoordinationOutcome, CoordinationScore, CoordinationValidationError, CoordinationVerdict,
};
// Re-export the canonical phase type so downstream consumers can depend on the
// engine crate alone without reaching into contracts for the common case.
pub use forge_core_contracts::Phase;
