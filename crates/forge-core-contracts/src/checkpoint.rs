//! Checkpoint / resume contract.
//!
//! A checkpoint is a typed manifest for resuming or rewinding a forge run at a
//! specific point in time. It does not own host-agent UI; it captures the stable
//! pointers needed by `forge checkpoint`, `forge resume`, and
//! `forge rewind-plan`-style flows.

use crate::common::{ClaimId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointContractDocument {
    pub schema_version: String,
    pub checkpoint_contract: CheckpointContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointContract {
    pub checkpoint_id: StableId,
    pub run_id: StableId,
    pub phase: String,
    pub captured_at: String,
    pub state_fingerprint: StateFingerprint,
    pub resumable: ResumablePoint,
}

impl CheckpointContract {
    /// Return `true` when the checkpoint has no failing checks and no blocked
    /// agent states.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.resumable.failing_checks.is_empty()
            && self
                .resumable
                .agent_states
                .iter()
                .all(|agent| !agent.blocked)
    }

    /// Suggest a deterministic rewind strategy.
    ///
    /// This is intentionally simple: prefer reverting to the captured worktree
    /// commit when there are failing checks, otherwise replay the WAL when a WAL
    /// position exists, and fall back to manual review when no machine-replayable
    /// anchor is available.
    #[must_use]
    pub fn suggest_rewind(&self) -> RewindStrategy {
        if self.state_fingerprint.worktree_commit.is_some()
            && !self.resumable.failing_checks.is_empty()
        {
            RewindStrategy::RevertToCommit
        } else if self.state_fingerprint.wal_position > 0 {
            RewindStrategy::ReplayWal
        } else {
            RewindStrategy::Manual
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateFingerprint {
    pub contract_state_hash: String,
    pub claim_set_hash: String,
    pub wal_position: u64,
    pub worktree_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResumablePoint {
    pub open_decisions: Vec<StableId>,
    pub failing_checks: Vec<String>,
    pub next_action: String,
    pub transcript_ref: Option<String>,
    pub agent_states: Vec<CheckpointAgentState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckpointAgentState {
    pub agent_id: StableId,
    pub role: String,
    pub last_action: String,
    pub claim_ref: Option<ClaimId>,
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RewindStrategy {
    RevertToCommit,
    ReplayWal,
    DiscardWorktree,
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_contract() -> CheckpointContract {
        CheckpointContract {
            checkpoint_id: StableId("checkpoint.run-1.0001".into()),
            run_id: StableId("run-1".into()),
            phase: "6-evolve".into(),
            captured_at: "2026-06-27T13:07:00Z".into(),
            state_fingerprint: StateFingerprint {
                contract_state_hash: "sha256:contracts".into(),
                claim_set_hash: "sha256:claims".into(),
                wal_position: 42,
                worktree_commit: Some("abcdef123456".into()),
            },
            resumable: ResumablePoint {
                open_decisions: vec![StableId("decision.router-risk".into())],
                failing_checks: Vec::new(),
                next_action: "continue wave 3 build".into(),
                transcript_ref: Some("transcripts/run-1.log".into()),
                agent_states: vec![CheckpointAgentState {
                    agent_id: StableId("worker.checkpoint".into()),
                    role: "worker".into(),
                    last_action: "implemented checkpoint contract".into(),
                    claim_ref: Some(ClaimId("claim.contracts.checkpoint".into())),
                    blocked: false,
                }],
            },
        }
    }

    #[test]
    fn serde_round_trip_preserves_checkpoint_document() {
        let document = CheckpointContractDocument {
            schema_version: "0.1".into(),
            checkpoint_contract: sample_contract(),
        };

        let encoded = serde_json::to_string(&document).expect("checkpoint serializes");
        let decoded: CheckpointContractDocument =
            serde_json::from_str(&encoded).expect("checkpoint deserializes");

        assert_eq!(document, decoded);
    }

    #[test]
    fn deny_unknown_fields_rejects_extra_document_key() {
        let value = json!({
            "schema_version": "0.1",
            "checkpoint_contract": sample_contract(),
            "unexpected": true
        });

        let err = serde_json::from_value::<CheckpointContractDocument>(value)
            .expect_err("unknown fields must be rejected");

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn is_clean_when_no_failures_or_blocked_agents() {
        let checkpoint = sample_contract();

        assert!(checkpoint.is_clean());
    }

    #[test]
    fn suggest_rewind_prefers_commit_when_failures_exist() {
        let mut checkpoint = sample_contract();
        checkpoint.resumable.failing_checks = vec!["cargo test failed".into()];

        assert_eq!(checkpoint.suggest_rewind(), RewindStrategy::RevertToCommit);
    }

    #[test]
    fn suggest_rewind_replays_wal_when_no_commit_revert_applies() {
        let mut checkpoint = sample_contract();
        checkpoint.state_fingerprint.worktree_commit = None;
        checkpoint.state_fingerprint.wal_position = 7;

        assert_eq!(checkpoint.suggest_rewind(), RewindStrategy::ReplayWal);
    }

    #[test]
    fn suggest_rewind_falls_back_to_manual_when_no_wal_anchor_exists() {
        let mut checkpoint = sample_contract();
        checkpoint.state_fingerprint.worktree_commit = None;
        checkpoint.state_fingerprint.wal_position = 0;

        assert_eq!(checkpoint.suggest_rewind(), RewindStrategy::Manual);
    }
}
