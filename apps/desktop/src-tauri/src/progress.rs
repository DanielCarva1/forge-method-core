//! Projection of an existing read-only record, not live agent progress.
use crate::project;
use serde::{Deserialize, Serialize};
use std::path::Path;

// One desktop process should not make its own read-only resume calls contend.
static RESUME_READ: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Deserialize)]
struct Resume {
    schema_version: String,
    project_id: String,
    current_phase: String,
    journey_guidance: JourneyGuidance,
    current_work: CurrentWork,
    active_objective: Option<ActiveObjective>,
    human_decisions: HumanDecisions,
    current_evaluation: CurrentEvaluation,
}
#[derive(Deserialize)]
struct JourneyGuidance {
    schema_version: String,
    authority: String,
    phase: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Absent,
    Current,
    Stale,
    Blocked,
    Completed,
    Abandoned,
}
#[derive(Deserialize)]
struct CurrentWork {
    schema_version: String,
    authority: String,
    status: WorkStatus,
    focus: Option<Focus>,
}
#[derive(Deserialize)]
struct ActiveObjective {
    authority_basis: String,
    revision: u64,
    revision_kind: RevisionKind,
    proposal: ObjectiveProposal,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RevisionKind {
    Initial,
    MaterialSupersession,
    NonMaterialClarification,
}
#[derive(Deserialize)]
struct ObjectiveProposal {
    outcome: String,
    constraints: Vec<String>,
    unacceptable_outcomes: Vec<String>,
    open_uncertainties: Vec<String>,
}
#[derive(Deserialize)]
struct HumanDecisions {
    recovered_pending: Vec<RecordedDecision>,
}
#[derive(Deserialize)]
struct RecordedDecision {
    status: String,
}
#[derive(Deserialize)]
struct CurrentEvaluation {
    candidate_decision_requests: Vec<CandidateDecision>,
}
#[derive(Deserialize, Serialize)]
struct CandidateDecision {
    question: String,
    alternatives: Vec<CandidateAlternative>,
    recommended_alternative_ref: String,
    blocking: bool,
}
#[derive(Deserialize, Serialize)]
struct CandidateAlternative {
    id: String,
    description: String,
    consequences: Vec<String>,
}
#[derive(Deserialize, Serialize)]
pub struct Focus {
    title: String,
    intended_outcome: String,
    current_activity: String,
    next_step: String,
    open_decision_count: usize,
    phase: String,
}
#[derive(Serialize)]
pub struct Progress {
    status: WorkStatus,
    phase: String,
    focus: Option<Focus>,
    accepted_direction: Option<AcceptedDirection>,
    recorded_pending_count: usize,
    suggested_questions: Vec<CandidateDecision>,
}
#[derive(Serialize)]
pub struct AcceptedDirection {
    outcome: String,
    constraints: Vec<String>,
    unacceptable_outcomes: Vec<String>,
    open_uncertainties: Vec<String>,
    revision: u64,
    revision_kind: RevisionKind,
    origin: &'static str,
}

#[derive(Deserialize)]
struct WorkflowReport {
    project_id: String,
    replacement_continuity: Option<ReportContinuity>,
}
#[derive(Deserialize)]
struct ReportContinuity {
    schema_version: String,
    binding: ReportBinding,
    objective_history: Vec<ReportRevision>,
}
#[derive(Deserialize)]
struct ReportBinding {
    project_id: String,
}
#[derive(Deserialize)]
#[serde(tag = "authority_kind", rename_all = "snake_case")]
enum ReportRevision {
    CooperativeSameOwner {
        active: bool,
        objective: ReportCooperativeObjective,
    },
    HumanIntent {
        active: bool,
        event: ReportHumanEvent,
    },
}
#[derive(Deserialize)]
struct ReportCooperativeObjective {
    revision: u64,
    revision_kind: RevisionKind,
    proposal: ObjectiveProposal,
    authority_basis: String,
    accepted_at_unix: u64,
}
#[derive(Deserialize)]
struct ReportHumanEvent {
    intent: ReportHumanIntent,
    accepted_at_unix: u64,
}
#[derive(Deserialize)]
struct ReportHumanIntent {
    revision: u64,
    desired_outcome: String,
    constraints: Vec<String>,
    unacceptable_outcomes: Vec<String>,
}
#[derive(Serialize)]
pub struct DirectionHistory {
    revisions: Vec<DirectionRevision>,
    earlier_count: usize,
}
#[derive(Serialize)]
struct DirectionRevision {
    active: bool,
    origin: &'static str,
    revision: u64,
    revision_kind: Option<RevisionKind>,
    outcome: String,
    constraints: Vec<String>,
    unacceptable_outcomes: Vec<String>,
    accepted_at_unix: u64,
}

fn project_direction_history(
    report: WorkflowReport,
    project_id: &str,
) -> Result<DirectionHistory, &'static str> {
    if report.project_id != project_id {
        return Err("O histórico não corresponde a este projeto.");
    }
    let Some(continuity) = report.replacement_continuity else {
        return Ok(DirectionHistory {
            revisions: Vec::new(),
            earlier_count: 0,
        });
    };
    if continuity.schema_version != "workflow_replacement_continuity_v1"
        || continuity.binding.project_id != project_id
    {
        return Err("O histórico retornado não corresponde ao projeto ou à versão esperada.");
    }
    let mut revisions = Vec::with_capacity(continuity.objective_history.len());
    let mut active_count = 0;
    for entry in continuity.objective_history {
        let revision = match entry {
            ReportRevision::CooperativeSameOwner { active, objective } => {
                if objective.authority_basis != "cooperative_same_owner"
                    || objective.revision == 0
                    || objective.proposal.outcome.trim().is_empty()
                {
                    return Err("O histórico de direções está incompleto.");
                }
                DirectionRevision {
                    active,
                    origin: "forge_cooperative_record",
                    revision: objective.revision,
                    revision_kind: Some(objective.revision_kind),
                    outcome: objective.proposal.outcome,
                    constraints: objective.proposal.constraints,
                    unacceptable_outcomes: objective.proposal.unacceptable_outcomes,
                    accepted_at_unix: objective.accepted_at_unix,
                }
            }
            ReportRevision::HumanIntent { active, event } => {
                if event.intent.revision == 0 || event.intent.desired_outcome.trim().is_empty() {
                    return Err("O histórico de direções está incompleto.");
                }
                DirectionRevision {
                    active,
                    origin: "human_intent_record",
                    revision: event.intent.revision,
                    revision_kind: None,
                    outcome: event.intent.desired_outcome,
                    constraints: event.intent.constraints,
                    unacceptable_outcomes: event.intent.unacceptable_outcomes,
                    accepted_at_unix: event.accepted_at_unix,
                }
            }
        };
        active_count += usize::from(revision.active);
        revisions.push(revision);
    }
    if active_count > 1 {
        return Err("O histórico tem mais de uma direção atual.");
    }
    const MAX_VISIBLE_REVISIONS: usize = 100;
    let earlier_count = revisions.len().saturating_sub(MAX_VISIBLE_REVISIONS);
    Ok(DirectionHistory {
        revisions: revisions.into_iter().skip(earlier_count).collect(),
        earlier_count,
    })
}

fn validate(value: Resume, project_id: &str) -> Result<Progress, &'static str> {
    if value.schema_version != "workflow_resume_summary_v10"
        || value.project_id != project_id
        || value.current_phase.trim().is_empty()
        || value.journey_guidance.schema_version != "product_journey_guidance_v2"
        || value.journey_guidance.authority != "advisory_read_only"
        || value.journey_guidance.phase != value.current_phase
        || value.current_work.schema_version != "current_work_context_v3"
        || value.current_work.authority != "advisory_read_only"
        || (!matches!(value.current_work.status, WorkStatus::Absent)
            && value.current_work.focus.is_none())
        || value.current_work.focus.as_ref().is_some_and(|focus| {
            focus.phase != value.current_phase
                || focus.title.trim().is_empty()
                || focus.intended_outcome.trim().is_empty()
                || focus.current_activity.trim().is_empty()
                || focus.next_step.trim().is_empty()
        })
        || value.active_objective.as_ref().is_some_and(|objective| {
            objective.authority_basis != "cooperative_same_owner"
                || objective.revision == 0
                || objective.proposal.outcome.trim().is_empty()
                || objective
                    .proposal
                    .constraints
                    .iter()
                    .any(|text| text.trim().is_empty())
                || objective
                    .proposal
                    .unacceptable_outcomes
                    .iter()
                    .any(|text| text.trim().is_empty())
                || objective
                    .proposal
                    .open_uncertainties
                    .iter()
                    .any(|text| text.trim().is_empty())
        })
        || value
            .human_decisions
            .recovered_pending
            .iter()
            .any(|decision| decision.status != "unresolved")
        || value
            .current_evaluation
            .candidate_decision_requests
            .iter()
            .any(|decision| {
                decision.question.trim().is_empty()
                    || !(2..=8).contains(&decision.alternatives.len())
                    || decision.recommended_alternative_ref.trim().is_empty()
                    || !decision
                        .alternatives
                        .iter()
                        .any(|alternative| alternative.id == decision.recommended_alternative_ref)
                    || decision.alternatives.iter().any(|alternative| {
                        alternative.id.trim().is_empty()
                            || alternative.description.trim().is_empty()
                            || alternative
                                .consequences
                                .iter()
                                .any(|text| text.trim().is_empty())
                    })
                    || decision
                        .alternatives
                        .iter()
                        .enumerate()
                        .any(|(index, alternative)| {
                            decision.alternatives[..index]
                                .iter()
                                .any(|prior| prior.id == alternative.id)
                        })
            })
    {
        return Err("O registro retornado não corresponde ao projeto ou à versão esperada.");
    }
    let accepted_direction = value.active_objective.map(|objective| AcceptedDirection {
        outcome: objective.proposal.outcome,
        constraints: objective.proposal.constraints,
        unacceptable_outcomes: objective.proposal.unacceptable_outcomes,
        open_uncertainties: objective.proposal.open_uncertainties,
        revision: objective.revision,
        revision_kind: objective.revision_kind,
        origin: "forge_cooperative_record",
    });
    Ok(Progress {
        status: value.current_work.status,
        phase: value.current_phase,
        focus: value.current_work.focus,
        accepted_direction,
        recorded_pending_count: value.human_decisions.recovered_pending.len(),
        suggested_questions: value
            .current_evaluation
            .candidate_decision_requests
            .into_iter()
            .collect(),
    })
}

#[tauri::command]
pub async fn inspect_progress(project_root: String) -> Result<Progress, &'static str> {
    let project = project::inspect_project(project_root).await?;
    let _read = RESUME_READ.lock().await;
    let resume = project::query(
        Path::new(&project.project_root),
        &["workflow", "resume"],
        "workflow.resume",
    )
    .await?;
    validate(resume, &project.project_id)
}

#[tauri::command]
pub async fn inspect_direction_history(
    project_root: String,
) -> Result<DirectionHistory, &'static str> {
    let project = project::inspect_project(project_root).await?;
    let _read = RESUME_READ.lock().await;
    let report: WorkflowReport = project::query_with_limit(
        Path::new(&project.project_root),
        &["workflow", "report"],
        "workflow.report",
        4 * 1024 * 1024,
    )
    .await?;
    project_direction_history(report, &project.project_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_projects_current_and_superseded_directions_with_origin() {
        let report: WorkflowReport = serde_json::from_value(serde_json::json!({
            "project_id":"project", "replacement_continuity": {
                "schema_version":"workflow_replacement_continuity_v1", "binding":{"project_id":"project"},
                "objective_history":[
                    {"authority_kind":"cooperative_same_owner","active":false,"objective":{"revision":1,"revision_kind":"initial","authority_basis":"cooperative_same_owner","accepted_at_unix":10,"proposal":{"outcome":"First direction","constraints":["Keep files"],"unacceptable_outcomes":[],"open_uncertainties":[]}}},
                    {"authority_kind":"cooperative_same_owner","active":true,"objective":{"revision":2,"revision_kind":"material_supersession","authority_basis":"cooperative_same_owner","accepted_at_unix":20,"proposal":{"outcome":"Current direction","constraints":[],"unacceptable_outcomes":["Lose files"],"open_uncertainties":[]}}}
                ]
            }
        })).unwrap();
        let history = project_direction_history(report, "project").unwrap();
        assert_eq!(history.revisions.len(), 2);
        assert!(!history.revisions[0].active);
        assert!(history.revisions[1].active);
        assert_eq!(history.revisions[1].outcome, "Current direction");
        assert_eq!(history.revisions[0].origin, "forge_cooperative_record");
        assert_eq!(history.earlier_count, 0);
    }

    #[test]
    fn history_rejects_wrong_project_or_conflicting_current_state() {
        let value = serde_json::json!({"project_id":"project","replacement_continuity":{"schema_version":"workflow_replacement_continuity_v1","binding":{"project_id":"project"},"objective_history":[]}});
        let report: WorkflowReport = serde_json::from_value(value.clone()).unwrap();
        assert!(project_direction_history(report, "other").is_err());
        let report: WorkflowReport = serde_json::from_value(value).unwrap();
        assert!(project_direction_history(report, "project")
            .unwrap()
            .revisions
            .is_empty());
        let conflicting: WorkflowReport = serde_json::from_value(serde_json::json!({
            "project_id":"project","replacement_continuity":{
                "schema_version":"workflow_replacement_continuity_v1","binding":{"project_id":"project"},
                "objective_history":[
                    {"authority_kind":"cooperative_same_owner","active":true,"objective":{"revision":1,"revision_kind":"initial","authority_basis":"cooperative_same_owner","accepted_at_unix":1,"proposal":{"outcome":"A","constraints":[],"unacceptable_outcomes":[],"open_uncertainties":[]}}},
                    {"authority_kind":"cooperative_same_owner","active":true,"objective":{"revision":2,"revision_kind":"initial","authority_basis":"cooperative_same_owner","accepted_at_unix":2,"proposal":{"outcome":"B","constraints":[],"unacceptable_outcomes":[],"open_uncertainties":[]}}}
                ]
            }
        })).unwrap();
        assert!(project_direction_history(conflicting, "project").is_err());
    }

    #[test]
    fn history_preserves_human_intent_origin_without_relabeling_it_cooperative() {
        let report: WorkflowReport = serde_json::from_value(serde_json::json!({
            "project_id":"project", "replacement_continuity": {
                "schema_version":"workflow_replacement_continuity_v1", "binding":{"project_id":"project"},
                "objective_history":[{"authority_kind":"human_intent","active":true,"event":{
                    "accepted_at_unix":40,"intent":{"revision":3,"desired_outcome":"Human direction","constraints":["Privacy"],"unacceptable_outcomes":[]}
                }}]
            }
        })).unwrap();
        let history = project_direction_history(report, "project").unwrap();
        assert_eq!(history.revisions[0].origin, "human_intent_record");
        assert_eq!(history.revisions[0].outcome, "Human direction");
        assert!(history.revisions[0].revision_kind.is_none());
    }
    fn response(status: &str) -> Resume {
        serde_json::from_value(serde_json::json!({"schema_version":"workflow_resume_summary_v10", "project_id":"project", "current_phase":"1-discovery", "journey_guidance":{"schema_version":"product_journey_guidance_v2", "authority":"advisory_read_only", "phase":"1-discovery"}, "current_work":{"schema_version":"current_work_context_v3", "authority":"advisory_read_only", "status":status, "focus":{"title":"Task", "intended_outcome":"Accepted outcome", "current_activity":"Recorded activity", "next_step":"Recorded next step", "open_decision_count":1, "phase":"1-discovery"}}, "active_objective":{"authority_basis":"cooperative_same_owner", "revision":2, "revision_kind":"material_supersession", "proposal":{"outcome":"Make a helpful app", "constraints":["Easy to use"], "unacceptable_outcomes":["Deleting work"], "open_uncertainties":[]}}, "human_decisions":{"recovered_pending":[{"status":"unresolved"}]}, "current_evaluation":{"candidate_decision_requests":[{"question":"Which direction?", "alternatives":[{"id":"simple", "description":"Simple path", "consequences":["Faster to try"]},{"id":"rich", "description":"Rich path", "consequences":["More work"]}], "recommended_alternative_ref":"simple", "blocking":true}]}})).unwrap()
    }
    #[test]
    fn preserves_recorded_states() {
        for status in [
            "absent",
            "current",
            "stale",
            "blocked",
            "completed",
            "abandoned",
        ] {
            assert!(validate(response(status), "project").is_ok());
        }
    }
    #[test]
    fn rejects_wrong_project_schema_and_missing_focus() {
        assert!(validate(response("current"), "another-project").is_err());
        let mut value = response("current");
        value.schema_version = "unknown".into();
        assert!(validate(value, "project").is_err());
        let mut value = response("current");
        value.current_work.focus = None;
        assert!(validate(value, "project").is_err());
    }

    #[test]
    fn projects_authoritative_phase_and_accepted_outcome() {
        let progress = validate(response("current"), "project").unwrap();
        assert_eq!(progress.phase, "1-discovery");
        assert_eq!(progress.focus.unwrap().intended_outcome, "Accepted outcome");
    }

    #[test]
    fn rejects_inconsistent_phase_and_blank_accepted_outcome() {
        let mut value = response("current");
        value.journey_guidance.phase = "2-specification".into();
        assert!(validate(value, "project").is_err());

        let mut value = response("current");
        value.current_work.focus.as_mut().unwrap().intended_outcome = " ".into();
        assert!(validate(value, "project").is_err());
    }

    #[test]
    fn separates_accepted_direction_from_pending_and_suggested_decisions() {
        let progress = validate(response("current"), "project").unwrap();
        let accepted = progress.accepted_direction.unwrap();
        assert_eq!(accepted.outcome, "Make a helpful app");
        assert_eq!(accepted.constraints, ["Easy to use"]);
        assert!(matches!(
            accepted.revision_kind,
            RevisionKind::MaterialSupersession
        ));
        assert_eq!(accepted.origin, "forge_cooperative_record");
        assert_eq!(progress.recorded_pending_count, 1);
        assert_eq!(progress.suggested_questions[0].question, "Which direction?");
        assert_eq!(
            progress.suggested_questions[0].alternatives[0].description,
            "Simple path"
        );
        assert!(progress.suggested_questions[0].blocking);
    }

    #[test]
    fn rejects_unverified_origin_and_blank_suggested_question() {
        let mut value = response("current");
        value.active_objective.as_mut().unwrap().authority_basis = "other".into();
        assert!(validate(value, "project").is_err());
        let mut value = response("current");
        value.current_evaluation.candidate_decision_requests[0].question = " ".into();
        assert!(validate(value, "project").is_err());
    }
    #[test]
    fn rejects_candidate_without_distinct_concrete_options_or_valid_recommendation() {
        let mut value = response("current");
        value.current_evaluation.candidate_decision_requests[0].alternatives[1].id =
            "simple".into();
        assert!(validate(value, "project").is_err());
        let mut value = response("current");
        value.current_evaluation.candidate_decision_requests[0].recommended_alternative_ref =
            "missing".into();
        assert!(validate(value, "project").is_err());
        let mut value = response("current");
        value.current_evaluation.candidate_decision_requests[0].alternatives[0].description =
            " ".into();
        assert!(validate(value, "project").is_err());
    }
}
