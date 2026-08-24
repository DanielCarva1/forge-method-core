use crate::{
    IsolationStatus, Phase, PrincipalId, RepoPath, StableId, WorkflowCooperativeHostProvenance,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CURRENT_WORK_CONTEXT_SCHEMA_VERSION: &str = "current_work_context_v3";
pub const CURRENT_WORK_DETAIL_SCHEMA_VERSION: &str = "current_work_detail_v3";
pub const LEGACY_WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION: &str = "work_focus_accept_input_v1";
pub const LEGACY_WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION: &str = "work_focus_update_input_v1";
pub const QUICK_CYCLE_WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION: &str = "work_focus_accept_input_v2";
pub const QUICK_CYCLE_WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION: &str = "work_focus_update_input_v2";
pub const WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION: &str = "work_focus_accept_input_v3";
pub const WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION: &str = "work_focus_update_input_v3";
pub const MAX_WORK_FOCUS_ACCEPT_INPUT_BYTES: u64 = 16 * 1_024;
pub const MAX_WORK_FOCUS_UPDATE_INPUT_BYTES: u64 = 16 * 1_024;
pub const MAX_WORK_FOCUS_TEXT_BYTES: usize = 1_024;
pub const MAX_WORK_FOCUS_LIST_ITEMS: usize = 16;
pub const MAX_WORK_FOCUS_EVENT_BYTES: usize = 16 * 1_024;
pub const MAX_CURRENT_WORK_SUMMARY_BYTES: usize = 8 * 1_024;
pub const MAX_CURRENT_WORK_DETAIL_BYTES: usize = 20 * 1_024;
pub const MAX_CURRENT_WORK_REFERENCE_ITEMS: usize = 16;
pub const MAX_CURRENT_WORK_SUMMARY_REFERENCE_ITEMS: usize = 4;
pub const MAX_CURRENT_WORK_SUMMARY_TEXT_BYTES: usize = 256;
pub const MAX_CURRENT_WORK_ARGV_ITEMS: usize = 16;
pub const MAX_CURRENT_WORK_ARG_BYTES: usize = 1_024;
pub const MAX_QUICK_CYCLE_COMPACTNESS_REASON_BYTES: usize = 512;
pub const MAX_QUICK_CYCLE_CLOSEOUT_SUMMARY_BYTES: usize = 768;
pub const MAX_QUICK_CYCLE_EXPANSION_REASON_BYTES: usize = 512;
pub const MAX_QUICK_CYCLE_EXPANSION_ITEMS: usize = 4;
pub const MAX_QUICK_CYCLE_EVIDENCE_ITEMS: usize = 2;
pub const MAX_COLLABORATION_LANES: usize = 8;
pub const MAX_COLLABORATION_DEPENDENCIES_PER_LANE: usize = 7;
pub const MAX_COLLABORATION_LANE_ID_BYTES: usize = 128;
pub const MAX_COLLABORATION_OUTCOME_BYTES: usize = 256;
pub const MAX_COLLABORATION_ISOLATION_ID_BYTES: usize = 128;

/// Exact objective revision to which one Work Focus belongs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkFocusObjectiveBinding {
    pub objective_id: StableId,
    pub objective_revision: u64,
    pub objective_digest: String,
    pub accepted_objective_record_digest: String,
    pub accepted_objective_record_sequence: u64,
    pub assurance_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowWorkFocusState {
    Active,
    Completed,
    Abandoned,
}

/// One accepted lifecycle-stage conclusion. Evidence remains owned by the
/// enclosing Work Focus and is referenced here only by canonical digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowQuickCycleCloseout {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_record_digests: Vec<String>,
}

/// Fixed lifecycle shape for proportional work. Missing entries are honest
/// partial progress, not silently inferred closeouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowQuickCycleStageCloseouts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_discovery: Option<WorkflowQuickCycleCloseout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_planning: Option<WorkflowQuickCycleCloseout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solution_definition: Option<WorkflowQuickCycleCloseout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<WorkflowQuickCycleCloseout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_delivery: Option<WorkflowQuickCycleCloseout>,
}

/// One accepted reason why proportional work had to expand. The canonical
/// Phase identifies where the extra treatment belongs without a second phase
/// model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowQuickCycleExpansion {
    pub phase: Phase,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_record_digests: Vec<String>,
}

/// Optional bounded continuity snapshot embedded in the existing Work Focus
/// event. Absence means not recorded and must never be reconstructed from prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowQuickCycleSnapshot {
    pub compactness_reason: String,
    pub stage_closeouts: WorkflowQuickCycleStageCloseouts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expansion_history: Vec<WorkflowQuickCycleExpansion>,
}

/// One independently useful outcome in a bounded collaboration plan. Runtime
/// ownership, worktree, path claims, and integration evidence remain in their
/// existing contracts and are deliberately not copied here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCollaborationLane {
    pub lane_id: StableId,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_id: Option<StableId>,
}

/// Optional task-local plan for cooperating agents. This is only the stable
/// plan: mutable execution state is derived later from claim, isolation, and
/// promotion owners rather than persisted a second time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCollaborationPlan {
    pub lanes: Vec<WorkflowCollaborationLane>,
}

/// Exact Work Focus state observed by the host before proposing a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowExpectedWorkFocus {
    Absent,
    Current { record_digest: String },
}

/// Host-authored meaning only. Objective, phase, clocks, admission coordinates,
/// lifecycle state, and advisory authority remain kernel-derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkFocusDraft {
    pub focus_id: StableId,
    pub title: String,
    pub intended_outcome: String,
    pub acceptance_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_goals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canonical_refs: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_area_refs: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_work_item_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_ref: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_reason: Option<String>,
    pub current_activity: String,
    pub next_step: String,
}

/// Optional task-local continuity written atomically with the surrounding Work
/// Focus change. The ledger remains the owner of referenced records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkFocusContinuityInput {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocker_record_digests: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_record_digests: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_cycle: Option<WorkflowQuickCycleSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration: Option<WorkflowCollaborationPlan>,
}

/// Closed public input for the first accepted Work Focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkFocusAcceptInput {
    pub schema_version: String,
    pub expected_snapshot_digest: String,
    pub expected_ledger_head_digest: String,
    pub expected_state_version: u64,
    pub expected_work_focus: WorkflowExpectedWorkFocus,
    pub focus: WorkflowWorkFocusDraft,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuity: Option<WorkflowWorkFocusContinuityInput>,
    pub recorded_by: PrincipalId,
    pub host_provenance: WorkflowCooperativeHostProvenance,
}

/// One explicit lifecycle change to the exact Work Focus observed by the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowWorkFocusChange {
    Supersede {
        focus: WorkflowWorkFocusDraft,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        continuity: Option<WorkflowWorkFocusContinuityInput>,
    },
    CheckpointQuickCycle {
        current_activity: String,
        next_step: String,
        continuity: WorkflowWorkFocusContinuityInput,
    },
    CheckpointCollaboration {
        current_activity: String,
        next_step: String,
        continuity: WorkflowWorkFocusContinuityInput,
    },
    Complete {
        completion_summary: String,
        next_step: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        continuity: Option<WorkflowWorkFocusContinuityInput>,
    },
    /// Replace the complete set of canonical ledger records explicitly related
    /// to the current focus. Empty lists intentionally clear prior bindings.
    BindReferences {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        blocker_record_digests: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidence_record_digests: Vec<String>,
    },
}

/// Closed public input for an exact Work Focus lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkFocusUpdateInput {
    pub schema_version: String,
    pub expected_snapshot_digest: String,
    pub expected_ledger_head_digest: String,
    pub expected_state_version: u64,
    pub expected_work_focus: WorkflowExpectedWorkFocus,
    pub change: WorkflowWorkFocusChange,
    pub recorded_by: PrincipalId,
    pub host_provenance: WorkflowCooperativeHostProvenance,
}

/// Work Focus is continuity guidance only. It grants no mutation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkAuthority {
    AdvisoryReadOnly,
}

/// One immutable, bounded semantic snapshot accepted into workflow governance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkFocusRecordedEvent {
    pub focus_id: StableId,
    pub objective: WorkflowWorkFocusObjectiveBinding,
    pub phase: Phase,
    pub state: WorkflowWorkFocusState,
    pub title: String,
    pub intended_outcome: String,
    pub acceptance_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_goals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canonical_refs: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_area_refs: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_work_item_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_ref: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_reason: Option<String>,
    pub current_activity: String,
    pub next_step: String,
    /// Exact `DecisionNeedRaised` ledger records accepted as blockers for this
    /// focus. Resolution is derived from later canonical ledger state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocker_record_digests: Vec<String>,
    /// Exact admitted `CooperativeEvidenceObserved` ledger records accepted as
    /// evidence for this focus. Evidence bytes remain owned by the ledger.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_record_digests: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_cycle: Option<WorkflowQuickCycleSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration: Option<WorkflowCollaborationPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_work_focus_record_digest: Option<String>,
    pub admission_ledger_head_digest: String,
    pub admission_state_version: u64,
    pub recorded_by: PrincipalId,
    pub host_provenance: WorkflowCooperativeHostProvenance,
    pub authority: WorkflowCurrentWorkAuthority,
    pub recorded_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkStatus {
    Absent,
    Current,
    Stale,
    Blocked,
    Completed,
    Abandoned,
}

/// Compact readback intended for ordinary workflow resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkContext {
    pub schema_version: String,
    pub authority: WorkflowCurrentWorkAuthority,
    pub status: WorkflowCurrentWorkStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<WorkflowCurrentWorkSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkSummary {
    pub focus_id: StableId,
    pub record_digest: String,
    pub objective: WorkflowWorkFocusObjectiveBinding,
    pub phase: Phase,
    pub title: String,
    pub intended_outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_work_item_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_ref: Option<StableId>,
    pub current_activity: String,
    pub next_step: String,
    pub open_decision_count: usize,
    pub blocker_count: usize,
    pub evidence_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_decision_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocker_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_cycle: Option<WorkflowCurrentWorkQuickCycleSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration: Option<WorkflowCurrentWorkCollaborationSummary>,
    pub detail_argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkQuickCycleState {
    ActiveCompact,
    ActiveExpanded,
    Completed,
    Abandoned,
}

/// Small derived Quick Cycle view for ordinary resume. Full accepted meaning
/// remains available only through Current Work detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkQuickCycleSummary {
    pub state: WorkflowCurrentWorkQuickCycleState,
    pub stage_closeout_count: usize,
    pub expansion_count: usize,
}

/// Small collaboration view for ordinary resume. Exact lanes and owner state
/// remain progressive detail; this summary carries counts and one next lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkCollaborationSummary {
    pub lane_count: usize,
    pub ready_lane_count: usize,
    pub active_lane_count: usize,
    pub blocked_lane_count: usize,
    pub integrated_lane_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_ready_lane: Option<WorkflowCurrentWorkCollaborationLaneSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkCollaborationLaneSummary {
    pub lane_id: StableId,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isolation_id: Option<StableId>,
}

/// Bounded progressive readback used only when the resume summary is insufficient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkDetail {
    pub schema_version: String,
    pub authority: WorkflowCurrentWorkAuthority,
    pub status: WorkflowCurrentWorkStatus,
    pub focus: WorkflowCurrentWorkDetailFocus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_decision_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocker_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor_detail_argv: Option<Vec<String>>,
}

/// Public detail projection. Ledger provenance and admission internals stay out
/// of this read-only interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkDetailFocus {
    pub focus_id: StableId,
    pub record_digest: String,
    pub objective: WorkflowWorkFocusObjectiveBinding,
    pub phase: Phase,
    pub state: WorkflowWorkFocusState,
    pub title: String,
    pub intended_outcome: String,
    pub acceptance_summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub non_goals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canonical_refs: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_area_refs: Vec<RepoPath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_work_item_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_ref: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_practice_reason: Option<String>,
    pub current_activity: String,
    pub next_step: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_work_focus_record_digest: Option<String>,
    pub admission_ledger_head_digest: String,
    pub admission_state_version: u64,
    pub recorded_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_cycle: Option<WorkflowQuickCycleSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration: Option<WorkflowCurrentWorkCollaborationDetail>,
}

/// Exact accepted collaboration plan plus read-only state joined from its
/// existing claim, isolation, and promotion owners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkCollaborationDetail {
    pub plan: WorkflowCollaborationPlan,
    pub lanes: Vec<WorkflowCurrentWorkCollaborationLaneDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkCollaborationLaneDetail {
    pub lane_id: StableId,
    pub state: WorkflowCurrentWorkCollaborationLaneState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<WorkflowCurrentWorkCollaborationOwnerDetail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_status: Option<WorkflowCurrentWorkCollaborationPromotionState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_receipt_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkCollaborationLaneState {
    Ready,
    Active,
    Blocked,
    Integrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCurrentWorkCollaborationOwnerDetail {
    pub isolation_id: StableId,
    pub agent_id: StableId,
    pub branch_name: String,
    pub worktree_path: RepoPath,
    pub isolation_status: IsolationStatus,
    pub isolation_validation: WorkflowCurrentWorkCollaborationIsolationValidation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_id: Option<StableId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_state: Option<WorkflowCurrentWorkCollaborationClaimState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkCollaborationClaimState {
    Live,
    Expired,
    NonActive,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkCollaborationIsolationValidation {
    Valid,
    ProposedNotCreated,
    RetiredWorktreeAbsent,
    Missing,
    Mismatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCurrentWorkCollaborationPromotionState {
    NotStarted,
    Recoverable,
    Completed,
    BlockedCorrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCurrentWorkValidationError {
    WrongSchema,
    StatusFocusMismatch,
    FieldBound,
    ListBound,
    InvalidDigest,
    PayloadTooLarge,
    DuplicateLaneId,
    DuplicateLaneDependency,
    UnknownLaneDependency,
    SelfLaneDependency,
    CyclicLaneDependency,
}

impl WorkflowCollaborationPlan {
    pub fn validate(&self) -> Result<(), WorkflowCurrentWorkValidationError> {
        if self.lanes.is_empty() || self.lanes.len() > MAX_COLLABORATION_LANES {
            return Err(WorkflowCurrentWorkValidationError::ListBound);
        }

        for (lane_index, lane) in self.lanes.iter().enumerate() {
            validate_collaboration_id(&lane.lane_id, MAX_COLLABORATION_LANE_ID_BYTES)?;
            if lane.outcome.trim().is_empty()
                || lane.outcome.as_bytes().len() > MAX_COLLABORATION_OUTCOME_BYTES
            {
                return Err(WorkflowCurrentWorkValidationError::FieldBound);
            }
            if let Some(isolation_id) = lane.isolation_id.as_ref() {
                validate_collaboration_id(isolation_id, MAX_COLLABORATION_ISOLATION_ID_BYTES)?;
            }
            if self.lanes[..lane_index]
                .iter()
                .any(|other| other.lane_id == lane.lane_id)
            {
                return Err(WorkflowCurrentWorkValidationError::DuplicateLaneId);
            }
            if lane.depends_on.len() > MAX_COLLABORATION_DEPENDENCIES_PER_LANE {
                return Err(WorkflowCurrentWorkValidationError::ListBound);
            }
            for (dependency_index, dependency) in lane.depends_on.iter().enumerate() {
                validate_collaboration_id(dependency, MAX_COLLABORATION_LANE_ID_BYTES)?;
                if dependency == &lane.lane_id {
                    return Err(WorkflowCurrentWorkValidationError::SelfLaneDependency);
                }
                if lane.depends_on[..dependency_index].contains(dependency) {
                    return Err(WorkflowCurrentWorkValidationError::DuplicateLaneDependency);
                }
                if !self.lanes.iter().any(|other| &other.lane_id == dependency) {
                    return Err(WorkflowCurrentWorkValidationError::UnknownLaneDependency);
                }
            }
        }

        let mut resolved = vec![false; self.lanes.len()];
        for _ in 0..self.lanes.len() {
            let next = self
                .lanes
                .iter()
                .enumerate()
                .position(|(lane_index, lane)| {
                    !resolved[lane_index]
                        && lane.depends_on.iter().all(|dependency| {
                            self.lanes
                                .iter()
                                .position(|other| &other.lane_id == dependency)
                                .is_some_and(|dependency_index| resolved[dependency_index])
                        })
                });
            let Some(next) = next else {
                return Err(WorkflowCurrentWorkValidationError::CyclicLaneDependency);
            };
            resolved[next] = true;
        }
        Ok(())
    }
}

impl WorkflowCurrentWorkContext {
    pub fn validate(&self) -> Result<(), WorkflowCurrentWorkValidationError> {
        if self.schema_version != CURRENT_WORK_CONTEXT_SCHEMA_VERSION {
            return Err(WorkflowCurrentWorkValidationError::WrongSchema);
        }
        if (self.status == WorkflowCurrentWorkStatus::Absent) != self.focus.is_none() {
            return Err(WorkflowCurrentWorkValidationError::StatusFocusMismatch);
        }
        if let Some(focus) = self.focus.as_ref() {
            validate_summary(focus)?;
        }
        validate_total(self, MAX_CURRENT_WORK_SUMMARY_BYTES)
    }
}

impl WorkflowCurrentWorkDetail {
    pub fn validate(&self) -> Result<(), WorkflowCurrentWorkValidationError> {
        if self.schema_version != CURRENT_WORK_DETAIL_SCHEMA_VERSION {
            return Err(WorkflowCurrentWorkValidationError::WrongSchema);
        }
        if self.status == WorkflowCurrentWorkStatus::Absent {
            return Err(WorkflowCurrentWorkValidationError::StatusFocusMismatch);
        }
        let state_matches = match self.status {
            WorkflowCurrentWorkStatus::Current | WorkflowCurrentWorkStatus::Blocked => {
                self.focus.state == WorkflowWorkFocusState::Active
            }
            WorkflowCurrentWorkStatus::Completed => {
                self.focus.state == WorkflowWorkFocusState::Completed
            }
            WorkflowCurrentWorkStatus::Abandoned => {
                self.focus.state == WorkflowWorkFocusState::Abandoned
            }
            // Stale describes the binding to current project state, independently
            // of the last recorded lifecycle state of that focus.
            WorkflowCurrentWorkStatus::Stale => true,
            WorkflowCurrentWorkStatus::Absent => false,
        };
        if !state_matches {
            return Err(WorkflowCurrentWorkValidationError::StatusFocusMismatch);
        }
        validate_detail_focus(&self.focus)?;
        validate_record_refs(&self.open_decision_refs)?;
        validate_record_refs(&self.blocker_refs)?;
        validate_record_refs(&self.evidence_refs)?;
        validate_optional_argv(self.predecessor_detail_argv.as_deref())?;
        if self.predecessor_detail_argv.is_some()
            && self.focus.previous_work_focus_record_digest.is_none()
        {
            return Err(WorkflowCurrentWorkValidationError::StatusFocusMismatch);
        }
        validate_total(self, MAX_CURRENT_WORK_DETAIL_BYTES)
    }
}

fn validate_summary(
    focus: &WorkflowCurrentWorkSummary,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    for value in [
        focus.focus_id.0.as_str(),
        focus.title.as_str(),
        focus.intended_outcome.as_str(),
        focus.current_activity.as_str(),
        focus.next_step.as_str(),
    ] {
        validate_text(value)?;
    }
    validate_objective(&focus.objective)?;
    validate_digest(&focus.record_digest)?;
    validate_optional_text(focus.external_work_item_ref.as_deref())?;
    validate_optional_text(
        focus
            .selected_practice_ref
            .as_ref()
            .map(|value| value.0.as_str()),
    )?;
    validate_record_refs(&focus.open_decision_refs)?;
    validate_record_refs(&focus.blocker_refs)?;
    validate_record_refs(&focus.evidence_refs)?;
    if let Some(quick_cycle) = focus.quick_cycle.as_ref() {
        if quick_cycle.stage_closeout_count > 5
            || quick_cycle.expansion_count > MAX_QUICK_CYCLE_EXPANSION_ITEMS
            || matches!(
                quick_cycle.state,
                WorkflowCurrentWorkQuickCycleState::ActiveCompact
            ) && quick_cycle.expansion_count != 0
            || matches!(
                quick_cycle.state,
                WorkflowCurrentWorkQuickCycleState::ActiveExpanded
            ) && quick_cycle.expansion_count == 0
            || matches!(
                quick_cycle.state,
                WorkflowCurrentWorkQuickCycleState::Completed
            ) && quick_cycle.stage_closeout_count != 5
        {
            return Err(WorkflowCurrentWorkValidationError::ListBound);
        }
    }
    if let Some(collaboration) = focus.collaboration.as_ref() {
        let classified = collaboration.ready_lane_count
            + collaboration.active_lane_count
            + collaboration.blocked_lane_count
            + collaboration.integrated_lane_count;
        if collaboration.lane_count == 0
            || collaboration.lane_count > MAX_COLLABORATION_LANES
            || classified != collaboration.lane_count
            || (collaboration.ready_lane_count == 0) != collaboration.next_ready_lane.is_none()
        {
            return Err(WorkflowCurrentWorkValidationError::ListBound);
        }
        if let Some(lane) = collaboration.next_ready_lane.as_ref() {
            validate_collaboration_id(&lane.lane_id, MAX_COLLABORATION_LANE_ID_BYTES)?;
            if lane.outcome.trim().is_empty()
                || lane.outcome.as_bytes().len() > MAX_COLLABORATION_OUTCOME_BYTES
            {
                return Err(WorkflowCurrentWorkValidationError::FieldBound);
            }
            if let Some(isolation_id) = lane.isolation_id.as_ref() {
                validate_collaboration_id(isolation_id, MAX_COLLABORATION_ISOLATION_ID_BYTES)?;
            }
        }
    }
    if focus.open_decision_refs.len() > focus.open_decision_count
        || focus.blocker_refs.len() > focus.blocker_count
        || focus.evidence_refs.len() > focus.evidence_count
        || focus.detail_argv.len() > MAX_CURRENT_WORK_ARGV_ITEMS
    {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    for argument in &focus.detail_argv {
        if argument.trim().is_empty() || argument.as_bytes().len() > MAX_CURRENT_WORK_ARG_BYTES {
            return Err(WorkflowCurrentWorkValidationError::FieldBound);
        }
    }
    Ok(())
}

fn validate_detail_focus(
    focus: &WorkflowCurrentWorkDetailFocus,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    for value in [
        focus.focus_id.0.as_str(),
        focus.title.as_str(),
        focus.intended_outcome.as_str(),
        focus.acceptance_summary.as_str(),
        focus.current_activity.as_str(),
        focus.next_step.as_str(),
    ] {
        validate_text(value)?;
    }
    validate_objective(&focus.objective)?;
    validate_digest(&focus.record_digest)?;
    validate_digest(&focus.admission_ledger_head_digest)?;
    // State version zero is the valid first append coordinate. Time zero is
    // not a valid accepted observation.
    if focus.recorded_at_unix == 0 {
        return Err(WorkflowCurrentWorkValidationError::FieldBound);
    }
    if let Some(digest) = focus.previous_work_focus_record_digest.as_deref() {
        validate_digest(digest)?;
    }
    if let Some(quick_cycle) = focus.quick_cycle.as_ref() {
        validate_quick_cycle_readback(quick_cycle)?;
    }
    if let Some(collaboration) = focus.collaboration.as_ref() {
        collaboration.plan.validate()?;
        if collaboration.lanes.len() != collaboration.plan.lanes.len() {
            return Err(WorkflowCurrentWorkValidationError::ListBound);
        }
        for (lane, accepted) in collaboration.lanes.iter().zip(&collaboration.plan.lanes) {
            if lane.lane_id != accepted.lane_id {
                return Err(WorkflowCurrentWorkValidationError::FieldBound);
            }
            if let Some(owner) = lane.owner.as_ref() {
                if accepted.isolation_id.as_ref() != Some(&owner.isolation_id) {
                    return Err(WorkflowCurrentWorkValidationError::FieldBound);
                }
                validate_collaboration_id(
                    &owner.isolation_id,
                    MAX_COLLABORATION_ISOLATION_ID_BYTES,
                )?;
                validate_collaboration_id(&owner.agent_id, MAX_COLLABORATION_ISOLATION_ID_BYTES)?;
                if owner.branch_name.trim().is_empty()
                    || owner.branch_name.as_bytes().len() > MAX_WORK_FOCUS_TEXT_BYTES
                    || owner.worktree_path.0.trim().is_empty()
                    || owner.worktree_path.0.as_bytes().len() > MAX_WORK_FOCUS_TEXT_BYTES
                {
                    return Err(WorkflowCurrentWorkValidationError::FieldBound);
                }
                if owner.claim_id.is_some() != owner.claim_state.is_some() {
                    return Err(WorkflowCurrentWorkValidationError::FieldBound);
                }
                if let Some(claim_id) = owner.claim_id.as_ref() {
                    validate_collaboration_id(claim_id, MAX_COLLABORATION_ISOLATION_ID_BYTES)?;
                }
            }
            if lane.promotion_receipt_digest.is_some()
                != (lane.promotion_status
                    == Some(WorkflowCurrentWorkCollaborationPromotionState::Completed))
            {
                return Err(WorkflowCurrentWorkValidationError::FieldBound);
            }
            if let Some(receipt) = lane.promotion_receipt_digest.as_deref() {
                validate_digest(receipt)?;
            }
            if lane.state == WorkflowCurrentWorkCollaborationLaneState::Integrated
                && lane.promotion_status
                    != Some(WorkflowCurrentWorkCollaborationPromotionState::Completed)
            {
                return Err(WorkflowCurrentWorkValidationError::FieldBound);
            }
        }
    }
    validate_optional_text(focus.external_work_item_ref.as_deref())?;
    validate_optional_text(
        focus
            .selected_practice_ref
            .as_ref()
            .map(|value| value.0.as_str()),
    )?;
    validate_optional_text(focus.selected_practice_reason.as_deref())?;
    if focus.selected_practice_ref.is_some() != focus.selected_practice_reason.is_some() {
        return Err(WorkflowCurrentWorkValidationError::FieldBound);
    }
    validate_text_list(&focus.non_goals)?;
    validate_repo_refs(&focus.canonical_refs)?;
    validate_repo_refs(&focus.affected_area_refs)
}

fn validate_optional_argv(
    argv: Option<&[String]>,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    let Some(argv) = argv else {
        return Ok(());
    };
    if argv.len() > MAX_CURRENT_WORK_ARGV_ITEMS
        || argv.iter().any(|value| {
            value.trim().is_empty() || value.as_bytes().len() > MAX_CURRENT_WORK_ARG_BYTES
        })
    {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    Ok(())
}

fn validate_quick_cycle_readback(
    quick_cycle: &WorkflowQuickCycleSnapshot,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    if quick_cycle.compactness_reason.trim().is_empty()
        || quick_cycle.compactness_reason.as_bytes().len()
            > MAX_QUICK_CYCLE_COMPACTNESS_REASON_BYTES
        || quick_cycle.expansion_history.len() > MAX_QUICK_CYCLE_EXPANSION_ITEMS
    {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    let closeouts = [
        quick_cycle.stage_closeouts.analysis_discovery.as_ref(),
        quick_cycle.stage_closeouts.product_planning.as_ref(),
        quick_cycle.stage_closeouts.solution_definition.as_ref(),
        quick_cycle.stage_closeouts.implementation.as_ref(),
        quick_cycle.stage_closeouts.validation_delivery.as_ref(),
    ];
    for closeout in closeouts.into_iter().flatten() {
        validate_quick_cycle_text_and_evidence(
            &closeout.summary,
            &closeout.evidence_record_digests,
            MAX_QUICK_CYCLE_CLOSEOUT_SUMMARY_BYTES,
        )?;
    }
    for expansion in &quick_cycle.expansion_history {
        validate_quick_cycle_text_and_evidence(
            &expansion.reason,
            &expansion.evidence_record_digests,
            MAX_QUICK_CYCLE_EXPANSION_REASON_BYTES,
        )?;
    }
    Ok(())
}

fn validate_quick_cycle_text_and_evidence(
    text: &str,
    evidence: &[String],
    max_text_bytes: usize,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    if text.trim().is_empty()
        || text.as_bytes().len() > max_text_bytes
        || evidence.len() > MAX_QUICK_CYCLE_EVIDENCE_ITEMS
    {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    validate_record_refs(evidence)
}

fn validate_objective(
    objective: &WorkflowWorkFocusObjectiveBinding,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    validate_text(&objective.objective_id.0)?;
    validate_digest(&objective.objective_digest)?;
    validate_digest(&objective.accepted_objective_record_digest)?;
    if objective.objective_revision == 0
        || objective.accepted_objective_record_sequence == 0
        || objective.assurance_epoch == 0
    {
        return Err(WorkflowCurrentWorkValidationError::FieldBound);
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), WorkflowCurrentWorkValidationError> {
    if value.trim().is_empty() || value.as_bytes().len() > MAX_WORK_FOCUS_TEXT_BYTES {
        return Err(WorkflowCurrentWorkValidationError::FieldBound);
    }
    Ok(())
}

fn validate_collaboration_id(
    value: &StableId,
    max_bytes: usize,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    if value.0.trim().is_empty() || value.0.as_bytes().len() > max_bytes {
        return Err(WorkflowCurrentWorkValidationError::FieldBound);
    }
    Ok(())
}

fn validate_optional_text(value: Option<&str>) -> Result<(), WorkflowCurrentWorkValidationError> {
    value.map_or(Ok(()), validate_text)
}

fn validate_text_list(values: &[String]) -> Result<(), WorkflowCurrentWorkValidationError> {
    if values.len() > MAX_CURRENT_WORK_REFERENCE_ITEMS {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    values.iter().try_for_each(|value| validate_text(value))
}

fn validate_repo_refs(values: &[RepoPath]) -> Result<(), WorkflowCurrentWorkValidationError> {
    if values.len() > MAX_CURRENT_WORK_REFERENCE_ITEMS {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    values.iter().try_for_each(|value| {
        validate_text(&value.0)?;
        let mut components = value.0.split('/');
        if value.0.starts_with(['/', '\\'])
            || value.0.contains('\\')
            || components.any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(WorkflowCurrentWorkValidationError::FieldBound);
        }
        Ok(())
    })
}

fn validate_record_refs(values: &[String]) -> Result<(), WorkflowCurrentWorkValidationError> {
    if values.len() > MAX_CURRENT_WORK_REFERENCE_ITEMS {
        return Err(WorkflowCurrentWorkValidationError::ListBound);
    }
    values.iter().try_for_each(|value| validate_digest(value))
}

fn validate_digest(value: &str) -> Result<(), WorkflowCurrentWorkValidationError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(WorkflowCurrentWorkValidationError::InvalidDigest);
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WorkflowCurrentWorkValidationError::InvalidDigest);
    }
    Ok(())
}

fn validate_total(
    value: &impl Serialize,
    maximum: usize,
) -> Result<(), WorkflowCurrentWorkValidationError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|_| WorkflowCurrentWorkValidationError::PayloadTooLarge)?;
    if encoded.len() > maximum {
        return Err(WorkflowCurrentWorkValidationError::PayloadTooLarge);
    }
    Ok(())
}
