//! Isolation engine — pure logic for layer-1 worktree isolation (S4.6).
//!
//! This module NEVER shells out to git. It validates isolation contracts,
//! detects cross-agent collisions (same branch or same worktree path), drives
//! the lifecycle state machine, and emits a deterministic merge plan. The host
//! agent (DC9) is the one that executes git; forge just describes + validates
//! + plans.
//!
//! `now` is injected everywhere (DD23) so the engine stays deterministic and
//! replayable.

use crate::rfc3339_to_unix;
use forge_core_contracts::common::StableId;
use forge_core_contracts::isolation::{
    GitAction, IsolationContract, IsolationError, IsolationStatus, MergePlan, MergePolicy,
    MergeStep,
};

/// Validate an isolation contract's intrinsic shape (everything that does not
/// depend on other contracts). Returns the first error found, or `Ok(())`.
///
/// Cross-contract checks (duplicate branch/path across siblings, claim-agent
/// consistency) live in [`detect_isolation_conflict`].
///
/// # Why this short-circuits (AGENTS.md carve-out)
///
/// This is an intentional exception to the repo's "validation accumulates
/// diagnostics, never short-circuits" rule. It is a **structural gate**, not a
/// diagnostic-accumulating pass: the caller (`isolation propose`) uses the
/// `Result` for control flow — a structurally invalid contract is rejected
/// before it reaches collision detection or persistence, and each
/// [`IsolationError`] variant maps to a distinct, user-facing rejection reason.
/// AGENTS.md explicitly permits bailing "only `?`-bail out of a validation
/// pass if an input is structurally unusable", which is exactly this case.
/// Migrating it onto the canonical `forge_core_validate::ValidationReport`
/// would also pull `forge-core-validate` into `forge-core-decisions` (a
/// different layer) and flatten ~10 distinct rejection reasons into generic
/// diagnostics — a net loss for the operator UX. Left as `Result` on purpose.
///
/// # Errors
///
/// Returns [`IsolationError::EmptyAgentId`] when `agent_id` is blank,
/// [`IsolationError::EmptyBaseRef`] when `base_ref` is blank, plus branch /
/// worktree-path / shell-metachar / `created_at` parse variants surfaced by
/// the dedicated validators.
pub fn validate_isolation_contract(c: &IsolationContract) -> Result<(), IsolationError> {
    if c.agent_id.0.trim().is_empty() {
        return Err(IsolationError::EmptyAgentId);
    }
    validate_branch_name(&c.branch_name)?;
    validate_worktree_path(&c.worktree_path.0)?;
    if c.base_ref.trim().is_empty() {
        return Err(IsolationError::EmptyBaseRef);
    }
    // Defense in depth (review S4.6 C1): base_ref flows into a shell command
    // the agent copy-pastes. Reject shell metacharacters at the engine
    // boundary so a malicious/typo'd ref can never become a second command.
    // shell_quote at emission is the second layer.
    if contains_shell_metachar(&c.base_ref) {
        return Err(IsolationError::ShellMetacharInField {
            field: "base_ref".to_string(),
            value: c.base_ref.clone(),
        });
    }
    // created_at must be parseable RFC3339 (fail-closed, like DD21).
    if rfc3339_to_unix(&c.created_at).is_none() {
        return Err(IsolationError::UnparseableCreatedAt {
            raw: c.created_at.clone(),
        });
    }
    Ok(())
}

/// Check a newly-proposed contract against the existing set. Returns the first
/// collision or `Ok(())`. Only NON-TERMINAL contracts (Proposed/Active/Merging)
/// can collide — a Merged/Abandoned isolation has released its branch and path.
///
/// # Errors
/// [`IsolationError::DuplicateBranch`] or [`IsolationError::DuplicateWorktreePath`].
/// Detect collisions between a `new` isolation contract and the live slice
/// of `existing` siblings (same branch or same worktree path).
///
/// # Errors
///
/// Returns [`IsolationError::DuplicateBranch`] when a live sibling already
/// owns the same branch name (case-insensitive), and
/// [`IsolationError::DuplicateWorktreePath`] when a live sibling already
/// owns the same normalized worktree path.
pub fn detect_isolation_conflict(
    new: &IsolationContract,
    existing: &[&IsolationContract],
) -> Result<(), IsolationError> {
    for old in existing {
        if !is_live(old) {
            continue;
        }
        // DD30 echo (review S4.6 M3): on a case-insensitive filesystem
        // (this repo's /mnt/c drvfs, plus Windows/macOS), git stores refs as
        // files under .git/refs/heads/, so `alice/s5` and `Alice/s5` are the
        // SAME ref. Compare case-INsensitively, mirroring normalize_path.
        if old.branch_name.eq_ignore_ascii_case(&new.branch_name) {
            return Err(IsolationError::DuplicateBranch {
                branch_name: new.branch_name.clone(),
                owner: old.agent_id.clone(),
            });
        }
        if normalize_path(&old.worktree_path.0) == normalize_path(&new.worktree_path.0) {
            return Err(IsolationError::DuplicateWorktreePath {
                worktree_path: new.worktree_path.0.clone(),
                owner: old.agent_id.clone(),
            });
        }
    }
    Ok(())
}

/// Validate a proposed lifecycle transition (DD35).
///
/// Legal transitions:
/// - Proposed -> Active | Abandoned
/// - Active   -> Merging | Abandoned
/// - Merging  -> Merged | Abandoned | Active (merge aborted, back to work)
/// - Merged/Abandoned are terminal (no outgoing).
///
/// # Errors
///
/// Returns [`IsolationError::IllegalTransition`] when `(from, to)` is not
/// in the legal transition table above.
pub fn transition_status(
    from: IsolationStatus,
    to: IsolationStatus,
) -> Result<IsolationStatus, IsolationError> {
    use IsolationStatus::{Abandoned, Active, Merged, Merging, Proposed};
    let legal = matches!(
        (from, to),
        (Proposed | Merging, Active)
            | (Proposed | Active | Merging, Abandoned)
            | (Active, Merging)
            | (Merging, Merged)
    );
    if legal {
        Ok(to)
    } else {
        Err(IsolationError::IllegalTransition { from, to })
    }
}

/// Build a deterministic merge plan for a live isolation contract (DD34).
///
/// The plan is a fixed sequence of git steps reflecting the chosen
/// [`MergePolicy`]. The host agent executes them in order and stops on the
/// first non-zero exit. `now_unix` is accepted for signature symmetry with the
/// rest of the engine (currently unused by the step list, reserved for future
/// timed actions).
#[must_use]
pub fn propose_merge(c: &IsolationContract, _now_unix: i64) -> MergePlan {
    let steps = match c.merge_policy {
        MergePolicy::Rebase => vec![
            MergeStep {
                action: GitAction::Checkout,
                args: vec![c.base_ref.clone()],
                rationale: format!("switch to the base ref '{}' before rebasing", c.base_ref),
            },
            MergeStep {
                action: GitAction::Fetch,
                args: vec![],
                rationale: "refresh the base ref in case it moved".to_string(),
            },
            MergeStep {
                action: GitAction::Rebase,
                args: vec![c.base_ref.clone(), c.branch_name.clone()],
                rationale: format!(
                    "replay '{}' onto the current '{}' (keeps history linear)",
                    c.branch_name, c.base_ref
                ),
            },
            MergeStep {
                action: GitAction::Checkout,
                args: vec![c.branch_name.clone()],
                rationale: "land on the rebased branch".to_string(),
            },
        ],
        MergePolicy::Merge => vec![
            MergeStep {
                action: GitAction::Checkout,
                args: vec![c.base_ref.clone()],
                rationale: format!("switch to '{}' to receive the merge", c.base_ref),
            },
            MergeStep {
                action: GitAction::Merge,
                args: vec!["--no-ff".to_string(), c.branch_name.clone()],
                rationale: format!(
                    "create a merge commit recording the '{}' branch",
                    c.branch_name
                ),
            },
        ],
        MergePolicy::Squash => vec![
            MergeStep {
                action: GitAction::Checkout,
                args: vec![c.base_ref.clone()],
                rationale: format!("switch to '{}' to receive the squash", c.base_ref),
            },
            MergeStep {
                action: GitAction::Squash,
                args: vec![c.branch_name.clone()],
                rationale: format!(
                    "fold all commits of '{}' into one staged change",
                    c.branch_name
                ),
            },
        ],
    };
    MergePlan {
        isolation_id: c.id.clone(),
        agent_id: c.agent_id.clone(),
        branch_name: c.branch_name.clone(),
        base_ref: c.base_ref.clone(),
        policy: c.merge_policy,
        steps,
    }
}

/// Canonical branch-name suggestion for an agent + scope (DD32).
#[must_use]
pub fn branch_name_for(agent: &StableId, scope: &str) -> String {
    // Slug both segments defensively: lowercase, swap non-alnum for `-`.
    let agent_slug = slug_branch_segment(&agent.0);
    let scope_slug = slug_branch_segment(scope);
    format!("{agent_slug}/{scope_slug}")
}

/// A contract is "live" (can collide / is mergeable) iff it is not terminal.
#[must_use]
pub fn is_live(c: &IsolationContract) -> bool {
    !matches!(
        c.status,
        IsolationStatus::Merged | IsolationStatus::Abandoned
    )
}

// ---------------------------------------------------------------------------
// internal validators
// ---------------------------------------------------------------------------

/// Does `s` contain a character that would let it break out of an unquoted
/// shell context? This is the defense-in-depth layer for review S4.6 C1:
/// values that flow into copy-pasted `git ...` commands are rejected here at
/// the engine boundary, AND shell-quoted at emission. Both layers must pass.
fn contains_shell_metachar(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c,
            ';' | '&' | '|' | '$' | '`' | '(' | ')' | '<' | '>' | '\n' | '{' | '}'
        )
    })
}

/// Same check returning a human reason for the validator error path.
///
/// Returns `Some("contains a shell metacharacter")` when the input contains
/// a shell metacharacter, `None` when it is safe. Replaces the legacy
/// `Result<_, String>` signature.
#[must_use]
fn shell_metachar_check(s: &str) -> Option<&'static str> {
    if contains_shell_metachar(s) {
        Some("contains a shell metacharacter")
    } else {
        None
    }
}

fn slug_branch_segment(segment: &str) -> String {
    let slug = segment
        .to_ascii_lowercase()
        .trim()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    slug.trim_matches('-').to_string()
}

/// Reject git-ref-illegal branch names. This is NOT a full git-check-ref-format
/// port — it blocks the high-risk shapes (`..`, whitespace, `:`, control chars,
/// leading `-`/`.`, empty). Exotic edge cases surface as git errors at execute
/// time, which is acceptable (fail-loud at the agent, not silently here).
fn validate_branch_name(name: &str) -> Result<(), IsolationError> {
    if name.trim().is_empty() {
        return Err(illegal_branch("empty", name));
    }
    if name.starts_with('-') || name.starts_with('.') {
        return Err(illegal_branch("must not start with '-' or '.'", name));
    }
    if name.contains("..") {
        return Err(illegal_branch("contains '..'", name));
    }
    if name.contains(':') {
        return Err(illegal_branch("contains ':'", name));
    }
    if name.contains(|ch: char| ch.is_whitespace()) {
        return Err(illegal_branch("contains whitespace", name));
    }
    if name.contains(|ch: char| ch.is_control()) {
        return Err(illegal_branch("contains control character", name));
    }
    // refname components must not be empty and must not end in `.lock`.
    for component in name.split('/') {
        if component.is_empty() {
            return Err(illegal_branch("contains empty component", name));
        }
        if component.to_ascii_lowercase().ends_with(".lock") {
            return Err(illegal_branch("component ends in '.lock'", name));
        }
    }
    // Defense in depth (review S4.6 C1): branch_name flows into a copy-pasted
    // `git worktree add -b <branch>` — reject shell metacharacters here too.
    if let Some(reason) = shell_metachar_check(name) {
        return Err(illegal_branch(reason, name));
    }
    Ok(())
}

/// Reject empty or traversal-prone worktree paths. Lexical only (DD29): we do
/// not resolve symlinks or realpath. Repo-root escape (`/abs` or `..` that
/// climbs past root) is left to git to refuse; we only block the obvious traps.
fn validate_worktree_path(path: &str) -> Result<(), IsolationError> {
    if path.trim().is_empty() {
        return Err(illegal_path("empty", path));
    }
    if path.starts_with('-') {
        return Err(illegal_path(
            "must not start with '-' (looks like a flag)",
            path,
        ));
    }
    // Defense in depth (review S4.6 C1): worktree_path flows into `git worktree
    // add ... <path>` and `cd <path>` — reject shell metacharacters.
    if let Some(reason) = shell_metachar_check(path) {
        return Err(illegal_path(reason, path));
    }
    Ok(())
}

/// Normalize a path for collision comparison: ASCII-lowercase + segment split
/// (mirrors `conflict_detection`'s DD30 casing decision, so `Worktrees/x` and
/// `worktrees/x` collide on case-insensitive filesystems).
fn normalize_path(path: &str) -> Vec<String> {
    // DD29 (echo of S4.5): lexical canonicalization — drop `.` and COLLAPSE
    // `..` against the preceding segment, so `../wt/x/../shared` and
    // `../wt/shared` normalize identically (review S4.6 M1: a `seg/../` hop
    // must not let an agent evade worktree-path uniqueness). Leading `..` is
    // preserved (repo-root escape is the spine's job, not the engine's).
    let mut out: Vec<String> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                // Collapse only if there is a non-`..` segment to pop; a
                // leading `..` (or excess `..`) is preserved as a segment.
                if out.last().is_some_and(|s| s != "..") {
                    out.pop();
                } else {
                    out.push("..".to_string());
                }
            }
            other => out.push(other.to_ascii_lowercase()),
        }
    }
    out
}

/// shorthand constructor for the most common error variant
fn illegal_branch(reason: &str, name: &str) -> IsolationError {
    IsolationError::IllegalBranchName {
        branch_name: name.to_string(),
        reason: reason.to_string(),
    }
}

fn illegal_path(reason: &str, path: &str) -> IsolationError {
    IsolationError::IllegalWorktreePath {
        worktree_path: path.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;
    const TS: &str = "2027-01-15T00:00:00Z";

    fn ok_contract(id: &str, agent: &str, branch: &str, path: &str) -> IsolationContract {
        IsolationContract {
            id: StableId(id.into()),
            agent_id: StableId(agent.into()),
            branch_name: branch.to_string(),
            worktree_path: RepoPath(path.into()),
            base_ref: "main".to_string(),
            created_at: TS.to_string(),
            status: IsolationStatus::Active,
            merge_policy: MergePolicy::Rebase,
            claim_id: None,
        }
    }

    use forge_core_contracts::common::RepoPath;

    // --- validate_isolation_contract ------------------------------------

    #[test]
    fn valid_contract_passes() {
        assert!(
            validate_isolation_contract(&ok_contract("i1", "alice", "alice/s5", "../wt/a")).is_ok()
        );
    }

    #[test]
    fn empty_agent_rejected() {
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.agent_id = StableId(String::new());
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::EmptyAgentId)
        ));
    }

    #[test]
    fn empty_base_ref_rejected() {
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.base_ref = String::new();
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::EmptyBaseRef)
        ));
    }

    #[test]
    fn unparseable_created_at_rejected() {
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.created_at = "not-a-date".to_string();
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::UnparseableCreatedAt { .. })
        ));
    }

    #[test]
    fn branch_with_double_dot_rejected() {
        let c = ok_contract("i1", "alice", "alice..s5", "../wt/a");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalBranchName { .. })
        ));
    }

    #[test]
    fn branch_leading_dot_rejected() {
        let c = ok_contract("i1", "alice", ".hidden", "../wt/a");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalBranchName { .. })
        ));
    }

    #[test]
    fn branch_whitespace_rejected() {
        let c = ok_contract("i1", "alice", "alice s5", "../wt/a");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalBranchName { .. })
        ));
    }

    #[test]
    fn branch_component_lock_suffix_rejected() {
        let c = ok_contract("i1", "alice", "alice/index.lock", "../wt/a");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalBranchName { .. })
        ));
    }

    #[test]
    fn branch_trailing_slash_rejected() {
        let c = ok_contract("i1", "alice", "alice/", "../wt/a");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalBranchName { .. })
        ));
    }

    #[test]
    fn branch_empty_component_rejected() {
        let c = ok_contract("i1", "alice", "alice//s5", "../wt/a");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalBranchName { .. })
        ));
    }

    #[test]
    fn path_empty_rejected() {
        let c = ok_contract("i1", "alice", "alice/s5", "");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalWorktreePath { .. })
        ));
    }

    #[test]
    fn path_leading_dash_rejected() {
        let c = ok_contract("i1", "alice", "alice/s5", "-danger");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalWorktreePath { .. })
        ));
    }

    // --- detect_isolation_conflict --------------------------------------

    #[test]
    fn disjoint_branches_and_paths_allowed() {
        let a = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        let b = ok_contract("i2", "bob", "bob/s6", "../wt/b");
        assert!(detect_isolation_conflict(&b, &[&a]).is_ok());
    }

    #[test]
    fn same_branch_blocked() {
        let a = ok_contract("i1", "alice", "shared/branch", "../wt/a");
        let b = ok_contract("i2", "bob", "shared/branch", "../wt/b");
        let err = detect_isolation_conflict(&b, &[&a]).unwrap_err();
        match err {
            IsolationError::DuplicateBranch { owner, .. } => {
                assert_eq!(owner.0, "alice");
            }
            other => panic!("expected DuplicateBranch, got {other:?}"),
        }
    }

    #[test]
    fn same_path_blocked() {
        let a = ok_contract("i1", "alice", "alice/s5", "../wt/shared");
        let b = ok_contract("i2", "bob", "bob/s6", "../wt/shared");
        assert!(matches!(
            detect_isolation_conflict(&b, &[&a]),
            Err(IsolationError::DuplicateWorktreePath { .. })
        ));
    }

    #[test]
    fn case_folded_path_collides() {
        // DD30 echo: ../wt/A and ../wt/a collide on case-insensitive FS.
        let a = ok_contract("i1", "alice", "alice/s5", "../wt/A");
        let b = ok_contract("i2", "bob", "bob/s6", "../wt/a");
        assert!(matches!(
            detect_isolation_conflict(&b, &[&a]),
            Err(IsolationError::DuplicateWorktreePath { .. })
        ));
    }

    #[test]
    fn terminal_contract_does_not_collide() {
        // A Merged isolation releases its branch+path.
        let mut a = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        a.status = IsolationStatus::Merged;
        let b = ok_contract("i2", "bob", "alice/s5", "../wt/a");
        assert!(detect_isolation_conflict(&b, &[&a]).is_ok());
    }

    #[test]
    fn abandoned_contract_does_not_collide() {
        let mut a = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        a.status = IsolationStatus::Abandoned;
        let b = ok_contract("i2", "bob", "alice/s5", "../wt/a");
        assert!(detect_isolation_conflict(&b, &[&a]).is_ok());
    }

    // --- transition_status ----------------------------------------------

    #[test]
    fn legal_transitions_succeed() {
        use IsolationStatus::*;
        for (from, to) in [
            (Proposed, Active),
            (Proposed, Abandoned),
            (Active, Merging),
            (Active, Abandoned),
            (Merging, Merged),
            (Merging, Abandoned),
            (Merging, Active),
        ] {
            assert_eq!(transition_status(from, to).unwrap(), to);
        }
    }

    #[test]
    fn terminal_states_reject_outgoing() {
        use IsolationStatus::*;
        for from in [Merged, Abandoned] {
            assert!(transition_status(from, Active).is_err());
            assert!(transition_status(from, Merging).is_err());
        }
    }

    #[test]
    fn illegal_skip_rejected() {
        use IsolationStatus::*;
        // cannot jump Proposed straight to Merging
        assert!(transition_status(Proposed, Merging).is_err());
        // cannot un-merge
        assert!(transition_status(Merged, Active).is_err());
    }

    // --- propose_merge --------------------------------------------------

    #[test]
    fn rebase_plan_has_expected_steps() {
        let c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        let plan = propose_merge(&c, NOW);
        assert_eq!(plan.policy, MergePolicy::Rebase);
        assert_eq!(plan.branch_name, "alice/s5");
        assert_eq!(plan.steps.len(), 4);
        assert!(matches!(plan.steps[2].action, GitAction::Rebase));
        // args are literal argv — rebase base branch
        assert_eq!(plan.steps[2].args, vec!["main", "alice/s5"]);
    }

    #[test]
    fn merge_plan_has_no_ff_merge() {
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.merge_policy = MergePolicy::Merge;
        let plan = propose_merge(&c, NOW);
        assert_eq!(plan.steps.len(), 2);
        assert!(matches!(plan.steps[1].action, GitAction::Merge));
        assert_eq!(plan.steps[1].args[0], "--no-ff");
    }

    #[test]
    fn squash_plan_squashes() {
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.merge_policy = MergePolicy::Squash;
        let plan = propose_merge(&c, NOW);
        assert_eq!(plan.steps.len(), 2);
        assert!(matches!(plan.steps[1].action, GitAction::Squash));
    }

    #[test]
    fn merge_plan_echoes_base_ref() {
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.base_ref = "release/2027".to_string();
        let plan = propose_merge(&c, NOW);
        assert_eq!(plan.base_ref, "release/2027");
        // first step checks out the base ref
        assert_eq!(plan.steps[0].args, vec!["release/2027"]);
    }

    // --- branch_name_for ------------------------------------------------

    #[test]
    fn branch_name_slug_normalizes() {
        assert_eq!(
            branch_name_for(&StableId("alice".into()), "S 5.0 BETA"),
            "alice/s-5-0-beta"
        );
    }

    #[test]
    fn branch_name_trims_dashes() {
        assert_eq!(branch_name_for(&StableId("bob".into()), "--x--"), "bob/x");
    }

    #[test]
    fn branch_name_slug_normalizes_agent_segment() {
        assert_eq!(
            branch_name_for(&StableId("Alice Agent;$(rm)".into()), "S5"),
            "alice-agent---rm/s5"
        );
    }

    // --- review S4.6 fixes ------------------------------------------------

    #[test]
    fn branch_with_shell_metachar_rejected() {
        // C1 defense-in-depth: `;`, `$()`, backtick must not reach the
        // copy-pasted git command. All three blocked at the engine boundary.
        for bad in ["alice;rm-rf", "alice$(x)", "alice`x`", "a&b", "a|b", "a<b"] {
            let c = ok_contract("i1", "alice", bad, "../wt/a");
            assert!(
                validate_isolation_contract(&c).is_err(),
                "branch '{bad}' must be rejected"
            );
        }
    }

    #[test]
    fn base_ref_with_shell_metachar_rejected() {
        // C1: base_ref was previously validated ONLY for non-emptiness.
        let mut c = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        c.base_ref = "main;touch pwned".to_string();
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::ShellMetacharInField { .. })
        ));
    }

    #[test]
    fn worktree_path_with_shell_metachar_rejected() {
        let c = ok_contract("i1", "alice", "alice/s5", "../wt/a;touch pwned");
        assert!(matches!(
            validate_isolation_contract(&c),
            Err(IsolationError::IllegalWorktreePath { .. })
        ));
    }

    #[test]
    fn path_collision_collapse_dotdot() {
        // M1: `../wt/x/../shared` MUST collide with `../wt/shared`.
        let a = ok_contract("i1", "alice", "alice/s5", "../wt/shared");
        let b = ok_contract("i2", "bob", "bob/s6", "../wt/x/../shared");
        assert!(matches!(
            detect_isolation_conflict(&b, &[&a]),
            Err(IsolationError::DuplicateWorktreePath { .. })
        ));
    }

    #[test]
    fn leading_dotdot_preserved_in_normalize() {
        // Leading `..` is preserved (repo-root escape is the spine's job).
        let a = ok_contract("i1", "alice", "alice/s5", "../../etc");
        let b = ok_contract("i2", "bob", "bob/s6", "../../etc");
        // both reduce to [.., .., etc] -> still collide (same path)
        assert!(matches!(
            detect_isolation_conflict(&b, &[&a]),
            Err(IsolationError::DuplicateWorktreePath { .. })
        ));
    }

    #[test]
    fn branch_collision_case_insensitive() {
        // M3: `alice/s5` and `Alice/s5` are the same git ref on /mnt/c.
        let a = ok_contract("i1", "alice", "alice/s5", "../wt/a");
        let b = ok_contract("i2", "bob", "Alice/s5", "../wt/b");
        assert!(matches!(
            detect_isolation_conflict(&b, &[&a]),
            Err(IsolationError::DuplicateBranch { .. })
        ));
    }
}
