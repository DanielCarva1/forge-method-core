use crate::{Phase, PrincipalId, RepoPath, StableId, WorkflowCooperativeHostProvenance};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CURRENT_WORK_CONTEXT_SCHEMA_VERSION: &str = "current_work_context_v1";
pub const CURRENT_WORK_DETAIL_SCHEMA_VERSION: &str = "current_work_detail_v1";
pub const WORK_FOCUS_ACCEPT_INPUT_SCHEMA_VERSION: &str = "work_focus_accept_input_v1";
pub const WORK_FOCUS_UPDATE_INPUT_SCHEMA_VERSION: &str = "work_focus_update_input_v1";
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
    pub current_activity: String,
    pub next_step: String,
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
    pub recorded_by: PrincipalId,
    pub host_provenance: WorkflowCooperativeHostProvenance,
}

/// One explicit lifecycle change to the exact Work Focus observed by the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowWorkFocusChange {
    Supersede {
        focus: WorkflowWorkFocusDraft,
    },
    Complete {
        completion_summary: String,
        next_step: String,
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
    pub detail_argv: Vec<String>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCurrentWorkValidationError {
    WrongSchema,
    StatusFocusMismatch,
    FieldBound,
    ListBound,
    InvalidDigest,
    PayloadTooLarge,
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
