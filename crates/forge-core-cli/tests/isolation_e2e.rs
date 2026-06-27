//! End-to-end CLI tests for layer-1 worktree isolation (S4.6).
//!
//! These exercise the run_* entry points (load->validate->persist->envelope)
//! against a real temp directory, mirroring `claim_e2e.rs`. They prove the
//! multi-agent isolation promise: two agents get disjoint worktrees/branches;
//! a duplicate is blocked; merge-back produces a deterministic git step list.

use forge_core_cli::isolation::{run_merge_plan, run_propose, run_status, run_transition};
use forge_core_contracts::common::StableId;
use forge_core_contracts::isolation::{IsolationStatus, MergePolicy};
use std::fs;
use std::path::{Path, PathBuf};

fn dir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("iso-e2e-{label}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

const NOW: i64 = 1_800_000_000;

fn propose(
    d: &Path,
    agent: &str,
    branch: &str,
    path: &str,
) -> forge_core_contracts::CliEnvelope<forge_core_cli::isolation::IsolationProposePayload> {
    run_propose(
        d,
        &StableId(agent.into()),
        branch,
        path,
        "main",
        MergePolicy::Rebase,
        None,
        &format!("iso-{agent}-{}", branch.replace('/', "-")),
        NOW,
    )
}

#[test]
fn two_agents_get_disjoint_isolations() {
    let d = dir("disjoint");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let b = propose(&d, "bob", "bob/s6", "../wt/b");
    assert!(a.ok, "alice propose must succeed");
    assert!(b.ok, "bob propose must succeed");

    let status = run_status(&d, None);
    assert!(status.ok);
    assert_eq!(status.data.as_ref().unwrap().total, 2);
}

#[test]
fn duplicate_branch_is_blocked() {
    let d = dir("dupbranch");
    let _ = propose(&d, "alice", "shared/feature", "../wt/a");
    let b = propose(&d, "bob", "shared/feature", "../wt/b");
    assert!(!b.ok, "duplicate branch must be blocked");
    assert_eq!(b.exit_code(), 2);
    assert!(b
        .error
        .as_ref()
        .unwrap()
        .message
        .contains("duplicate_branch"));
}

#[test]
fn duplicate_worktree_path_is_blocked() {
    let d = dir("duppath");
    let _ = propose(&d, "alice", "alice/s5", "../wt/shared");
    let b = propose(&d, "bob", "bob/s6", "../wt/shared");
    assert!(!b.ok, "duplicate worktree path must be blocked");
    assert!(b
        .error
        .as_ref()
        .unwrap()
        .message
        .contains("duplicate_worktree_path"));
}

#[test]
fn propose_returns_suggested_git_commands() {
    let d = dir("suggest");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let cmds = &a.data.as_ref().unwrap().suggested_git_commands;
    assert!(
        cmds.iter().any(|c| c.contains("worktree add")),
        "must suggest worktree add"
    );
    // worktree_path is now shell-quoted (review S4.6 C1): cd '../wt/a'
    assert!(
        cmds.iter().any(|c| c.contains("cd '../wt/a'")),
        "must suggest cd into worktree"
    );
}

#[test]
fn merge_plan_produces_deterministic_rebase_steps() {
    let d = dir("mergeplan");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let id = StableId(a.data.unwrap().isolation.id.0);
    let plan = run_merge_plan(&d, &id, NOW);
    assert!(plan.ok);
    let steps = &plan.data.as_ref().unwrap().steps;
    assert_eq!(steps.len(), 4, "rebase policy => 4 steps");
    // Rebase step must reference both base ref and branch.
    assert!(steps.iter().any(|s| {
        use forge_core_contracts::isolation::GitAction::*;
        matches!(s.action, Rebase) && s.args.contains(&"main".to_string())
    }));
}

#[test]
fn full_lifecycle_active_to_merged_releases_branch() {
    let d = dir("lifecycle");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let id = StableId(a.data.unwrap().isolation.id.0);

    // Proposed -> Active
    let t = run_transition(&d, &id, IsolationStatus::Active, NOW);
    assert!(t.ok);
    // Active -> Merging (emits merge commands)
    let t = run_transition(&d, &id, IsolationStatus::Merging, NOW);
    assert!(t.ok);
    assert!(!t.data.as_ref().unwrap().suggested_git_commands.is_empty());
    // Merging -> Merged (terminal)
    let t = run_transition(&d, &id, IsolationStatus::Merged, NOW);
    assert!(t.ok);

    // Branch + path are now free for another agent.
    let b = propose(&d, "bob", "alice/s5", "../wt/a");
    assert!(
        b.ok,
        "merged isolation releases its branch and worktree path"
    );
}

#[test]
fn illegal_transition_blocked() {
    let d = dir("illegal");
    let a = propose(&d, "alice", "alice/s5", "../wt/a");
    let id = StableId(a.data.unwrap().isolation.id.0);
    // Proposed -> Merged is illegal (skip Active/Merging).
    let t = run_transition(&d, &id, IsolationStatus::Merged, NOW);
    assert!(!t.ok);
    assert_eq!(t.exit_code(), 2);
}

#[test]
fn merge_plan_unknown_id_is_invalid_shape() {
    let d = dir("unknown");
    let plan = run_merge_plan(&d, &StableId("does-not-exist".into()), NOW);
    assert!(!plan.ok);
    assert_eq!(plan.exit_code(), 3); // InvalidDecisionShape
}

#[test]
fn status_persists_across_invocations() {
    // The contract YAML is the materialized state (DD22): a fresh process
    // loading the same dir sees the same isolations.
    let d = dir("persist");
    let _ = propose(&d, "alice", "alice/s5", "../wt/a");
    let _ = propose(&d, "bob", "bob/s6", "../wt/b");

    let status = run_status(&d, None);
    assert_eq!(status.data.as_ref().unwrap().total, 2);

    let ids: Vec<_> = status
        .data
        .unwrap()
        .active
        .into_iter()
        .map(|s| s.agent_id)
        .collect();
    assert!(ids.contains(&"alice".to_string()));
    assert!(ids.contains(&"bob".to_string()));
}
