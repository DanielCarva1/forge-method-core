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
    {
        return Err("O registro retornado não corresponde ao projeto ou à versão esperada.");
    }
    Ok(Progress {
        status: value.current_work.status,
        phase: value.current_phase,
        focus: value.current_work.focus,
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

#[cfg(test)]
mod tests {
    use super::*;
    fn response(status: &str) -> Resume {
        serde_json::from_value(serde_json::json!({"schema_version":"workflow_resume_summary_v10", "project_id":"project", "current_phase":"1-discovery", "journey_guidance":{"schema_version":"product_journey_guidance_v2", "authority":"advisory_read_only", "phase":"1-discovery"}, "current_work":{"schema_version":"current_work_context_v3", "authority":"advisory_read_only", "status":status, "focus":{"title":"Task", "intended_outcome":"Accepted outcome", "current_activity":"Recorded activity", "next_step":"Recorded next step", "open_decision_count":1, "phase":"1-discovery"}}})).unwrap()
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
}
