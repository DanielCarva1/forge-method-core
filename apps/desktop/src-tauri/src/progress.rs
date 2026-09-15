//! Projection of an existing read-only record, not live agent progress.
use crate::project;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize)]
struct Resume {
    schema_version: String,
    project_id: String,
    current_work: Progress,
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
#[derive(Deserialize, Serialize)]
pub struct Progress {
    schema_version: String,
    authority: String,
    status: WorkStatus,
    focus: Option<Focus>,
}
#[derive(Deserialize, Serialize)]
pub struct Focus {
    title: String,
    current_activity: String,
    next_step: String,
    open_decision_count: usize,
}

fn validate(value: Resume, project_id: &str) -> Result<Progress, &'static str> {
    if value.schema_version != "workflow_resume_summary_v10"
        || value.project_id != project_id
        || value.current_work.schema_version != "current_work_context_v3"
        || value.current_work.authority != "advisory_read_only"
        || (!matches!(value.current_work.status, WorkStatus::Absent)
            && value.current_work.focus.is_none())
    {
        return Err("O registro retornado não corresponde ao projeto ou à versão esperada.");
    }
    Ok(value.current_work)
}

#[tauri::command]
pub async fn inspect_progress(project_root: String) -> Result<Progress, &'static str> {
    let project = project::inspect_project(project_root).await?;
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
        serde_json::from_value(serde_json::json!({"schema_version":"workflow_resume_summary_v10", "project_id":"project", "current_work":{"schema_version":"current_work_context_v3", "authority":"advisory_read_only", "status":status, "focus":{"title":"Task", "current_activity":"Recorded activity", "next_step":"Recorded next step", "open_decision_count":1}}})).unwrap()
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
}
