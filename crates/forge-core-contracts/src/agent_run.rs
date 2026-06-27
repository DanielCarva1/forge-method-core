use crate::common::{ClaimId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentRunContractDocument {
    pub schema_version: String,
    pub agent_run_contract: AgentRunContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentRunContract {
    pub run_id: StableId,
    pub root_agent: StableId,
    pub started_at: String,
    pub agents: Vec<AgentRunEntry>,
    pub edges: Vec<RunDependency>,
    pub summary: RunSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentRunEntry {
    pub agent_id: StableId,
    pub role: AgentRole,
    pub state: AgentState,
    pub current_task: Option<String>,
    pub last_message: Option<String>,
    pub claim_ref: Option<ClaimId>,
    pub worktree_ref: Option<String>,
    pub started_at: String,
    pub last_heartbeat_at: String,
    pub retries: u8,
    pub blocked_reason: Option<StableId>,
    pub handoff_to: Option<StableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Driver,
    Planner,
    Researcher,
    Reviewer,
    Builder,
    Oracle,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Pending,
    Running,
    Idle,
    Blocked,
    Retrying,
    HandedOff,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunDependency {
    pub from_agent: StableId,
    pub to_agent: StableId,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Blocks,
    WaitsFor,
    HandsOffTo,
    ParallelWith,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunSummary {
    pub total: usize,
    pub running: usize,
    pub blocked: usize,
    pub completed: usize,
    pub failed: usize,
    pub healthy: bool,
}

impl AgentRunContract {
    pub fn blocked_agents(&self) -> impl Iterator<Item = &AgentRunEntry> {
        self.agents
            .iter()
            .filter(|agent| agent.state == AgentState::Blocked)
    }

    pub fn live_agents(&self) -> impl Iterator<Item = &AgentRunEntry> {
        self.agents.iter().filter(|agent| {
            matches!(
                agent.state,
                AgentState::Running | AgentState::Retrying | AgentState::Idle
            )
        })
    }

    pub fn recompute_summary(&mut self) {
        let total = self.agents.len();
        let running = self
            .agents
            .iter()
            .filter(|agent| agent.state == AgentState::Running)
            .count();
        let blocked = self.blocked_agents().count();
        let completed = self
            .agents
            .iter()
            .filter(|agent| agent.state == AgentState::Completed)
            .count();
        let failed = self
            .agents
            .iter()
            .filter(|agent| agent.state == AgentState::Failed)
            .count();
        let healthy = failed == 0 && blocked < total;

        self.summary = RunSummary {
            total,
            running,
            blocked,
            completed,
            failed,
            healthy,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable_id(value: &str) -> StableId {
        StableId(value.to_owned())
    }

    fn claim_id(value: &str) -> ClaimId {
        ClaimId(value.to_owned())
    }

    fn entry(agent_id: &str, role: AgentRole, state: AgentState) -> AgentRunEntry {
        AgentRunEntry {
            agent_id: stable_id(agent_id),
            role,
            state,
            current_task: Some(format!("task for {agent_id}")),
            last_message: Some(format!("last message from {agent_id}")),
            claim_ref: None,
            worktree_ref: Some(format!("/tmp/{agent_id}")),
            started_at: "2026-06-27T00:00:00Z".to_owned(),
            last_heartbeat_at: "2026-06-27T00:01:00Z".to_owned(),
            retries: 0,
            blocked_reason: None,
            handoff_to: None,
        }
    }

    fn sample_run() -> AgentRunContractDocument {
        let mut builder = entry("builder-1", AgentRole::Builder, AgentState::Blocked);
        builder.claim_ref = Some(claim_id("claim.story.e1.builder"));
        builder.blocked_reason = Some(stable_id("waiting_for_review"));

        AgentRunContractDocument {
            schema_version: "0.1".to_owned(),
            agent_run_contract: AgentRunContract {
                run_id: stable_id("run-wave-3"),
                root_agent: stable_id("driver-1"),
                started_at: "2026-06-27T00:00:00Z".to_owned(),
                agents: vec![
                    entry("driver-1", AgentRole::Driver, AgentState::Running),
                    builder,
                    entry("researcher-1", AgentRole::Researcher, AgentState::Idle),
                    entry("reviewer-1", AgentRole::Reviewer, AgentState::Retrying),
                ],
                edges: vec![
                    RunDependency {
                        from_agent: stable_id("driver-1"),
                        to_agent: stable_id("builder-1"),
                        kind: DependencyKind::Blocks,
                    },
                    RunDependency {
                        from_agent: stable_id("builder-1"),
                        to_agent: stable_id("reviewer-1"),
                        kind: DependencyKind::HandsOffTo,
                    },
                ],
                summary: RunSummary {
                    total: 4,
                    running: 1,
                    blocked: 1,
                    completed: 0,
                    failed: 0,
                    healthy: true,
                },
            },
        }
    }

    #[test]
    fn agent_run_round_trips_with_four_agents_and_two_edges() {
        let document = sample_run();

        let encoded = serde_json::to_string(&document).expect("serialize agent run document");
        let decoded: AgentRunContractDocument =
            serde_json::from_str(&encoded).expect("deserialize agent run document");

        assert_eq!(decoded, document);
        assert_eq!(decoded.agent_run_contract.agents.len(), 4);
        assert_eq!(decoded.agent_run_contract.edges.len(), 2);
    }

    #[test]
    fn agent_run_document_denies_unknown_fields() {
        let mut value = serde_json::to_value(sample_run()).expect("encode to value");
        value
            .as_object_mut()
            .expect("document object")
            .insert("unexpected".to_owned(), serde_json::json!(true));

        let err = serde_json::from_value::<AgentRunContractDocument>(value)
            .expect_err("unknown document field must be rejected");

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn recompute_summary_tracks_manual_state_edits() {
        let mut run = sample_run().agent_run_contract;
        run.summary = RunSummary {
            total: 0,
            running: 0,
            blocked: 0,
            completed: 0,
            failed: 0,
            healthy: false,
        };

        run.recompute_summary();
        assert_eq!(run.summary.total, 4);
        assert_eq!(run.summary.running, 1);
        assert_eq!(run.summary.blocked, 1);
        assert_eq!(run.summary.completed, 0);
        assert_eq!(run.summary.failed, 0);
        assert!(run.summary.healthy);

        run.agents[1].state = AgentState::Completed;
        run.agents[2].state = AgentState::Failed;
        run.agents[3].state = AgentState::Completed;

        run.recompute_summary();
        assert_eq!(run.summary.total, 4);
        assert_eq!(run.summary.running, 1);
        assert_eq!(run.summary.blocked, 0);
        assert_eq!(run.summary.completed, 2);
        assert_eq!(run.summary.failed, 1);
        assert!(!run.summary.healthy);
    }

    #[test]
    fn blocked_and_live_agent_helpers_return_expected_subsets() {
        let run = sample_run().agent_run_contract;

        let blocked_ids: Vec<&str> = run
            .blocked_agents()
            .map(|agent| agent.agent_id.0.as_str())
            .collect();
        let live_ids: Vec<&str> = run
            .live_agents()
            .map(|agent| agent.agent_id.0.as_str())
            .collect();

        assert_eq!(blocked_ids, vec!["builder-1"]);
        assert_eq!(live_ids, vec!["driver-1", "researcher-1", "reviewer-1"]);
    }
}
