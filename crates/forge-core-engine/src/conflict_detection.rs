//! Write-before-claim conflict detection — the engine half of layer-2 prevention.
//!
//! A claim is a *semantic* reservation (S4.3): agent A declares "I own scope X
//! for the next TTL". That reservation is meaningless against physical writes
//! unless the engine can answer one question:
//!
//! > "Is this write target inside a path that another agent currently claims?"
//!
//! This module answers it. It is **pure** — given the write set, the writer's
//! id, the active claims, and an injected `now` (epoch seconds), it returns a
//! typed [`WriteCheck`]. No IO. The CLI / runtime layers call it before letting
//! an effect land.
//!
//! ## The three outcomes
//!
//! - [`WriteCheck::Blocked`] — at least one target sits inside a **live** claim
//!   held by a *different* agent. The write MUST NOT proceed. The conflict is
//!   reported with a typed [`ConflictCode`] so the offending agent can
//!   self-correct (acquire its own scope, or wait for a handoff).
//! - [`WriteCheck::Ok`] with `governed_by_self` — the target is inside a live
//!   claim held by the *same* writer. Authorized: the agent is writing within
//!   its own reserved scope.
//! - [`WriteCheck::Ok`] with `ungoverned` ? no live claim covers the target.
//!   Whether an *ungoverned* write is allowed is a **policy** decision
//!   (require-claim vs allow-it), not this classifier's decision (DD8/DD19).
//!   The engine reports the fact; the policy layer decides. The Forge CLI's
//!   default policy is strict: ungoverned writes are rejected.
//!
//! ## Hard rules the engine enforces (non-delegable)
//!
//! 1. A live claim held by ANOTHER agent always blocks. The engine never lets a
//!    writer clobber a peer's reserved scope.
//! 2. Only **live** claims (status `Active` or `Stale` AND not past
//!    `expires_at`) block. Expired / Released / `HandoffRequired` claims have no
//!    active owner, so they do not block — consistent with the fail-closed
//!    lease rule (DD21): a dead lease is never treated as authority.
//!
//! ## Design decisions
//!
//! - **DD26 — path containment is path-segment prefix match.** A claim path
//!   covers a target iff the target equals it (exact file) or the target is
//!   strictly inside it as a directory. We match on `/`-separated *segments*,
//!   never raw string prefixes — otherwise a claim on `src/ev` would wrongly
//!   "cover" `src/evil.rs`. This is the classic containment trap.
//! - **DD27 — only live claims block** (above).
//! - **DD28 — a claim with empty `scope.paths` governs no path.** Project- or
//!   area-scoped claims coordinate generally (driver authority, fleet mode)
//!   but do not block individual writes. Symlinks are not resolved here (pure
//!   logic); the filesystem / integrity spine resolves them when the write
//!   actually lands.
//! - **DD29 — lexical path normalization.** `.` segments are dropped and `..`
//!   is collapsed lexically before matching, but leading/excess `..` segments
//!   are preserved so repo-root escapes cannot normalize into governed paths.
//!   Thus a write to `contracts/claims/../secret` is NOT treated as inside a
//!   claim on `contracts/claims`, and `../../claimed` is NOT treated as
//!   `claimed`. This is pure-string normalization (no IO / no symlink
//!   resolution); it defends against obvious lexical escapes, and the spine is
//!   the final authority on the real on-disk path.

use forge_core_contracts::{
    claim::ClaimContract,
    common::{ClaimId, RepoPath, StableId},
    tool_effect::ConflictCode,
};

use crate::claim_engine::is_live;

/// One blocked target, fully attributed so the offending agent can self-correct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDetail {
    /// The write target that collided with another agent's claim.
    pub blocked_path: RepoPath,
    /// The id of the claim that owns the scope (e.g. `claim.story.S4.5`).
    pub blocking_claim_id: ClaimId,
    /// The agent that currently holds the blocking claim.
    pub claimant: StableId,
    /// Why the write was blocked (typed, DD10 self-correction surface).
    pub conflict_code: ConflictCode,
}

/// The verdict of checking a write set against the active claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteCheck {
    /// Every target is clear to write.
    ///
    /// `governed_by_self` lists targets that sit inside one of the writer's own
    /// live claims (authorized). `ungoverned` lists targets covered by no live
    /// claim at all — allowed-or-not is a policy call, not an engine call.
    Ok {
        governed_by_self: Vec<RepoPath>,
        ungoverned: Vec<RepoPath>,
    },
    /// At least one target sits inside another agent's live claim.
    ///
    /// `blocks` lists every colliding target (full diagnostic, not just the
    /// first) so the writer can plan around all conflicts at once. The write
    /// MUST NOT proceed.
    Blocked { blocks: Vec<BlockDetail> },
}

impl WriteCheck {
    /// `true` iff the check blocks the write (layer-2 prevention fired).
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked { .. })
    }
}

/// Check a multi-path write set against the active claims.
///
/// For each target the engine asks: is any **live** claim that covers this path
/// held by an agent OTHER than `writer_agent_id`? If so, that target is
/// [`BlockDetail`] and the whole write is [`WriteCheck::Blocked`].
///
/// Determinism: claims are inspected in sorted order (by claim id) so the set
/// of reported `blocks` is stable regardless of input ordering.
///
/// # Arguments
/// * `targets` - the paths the writer intends to modify.
/// * `writer_agent_id` - the agent attempting the write.
/// * `claims` - all known claims (the coordination bus view).
/// * `now_unix` - injected time (epoch seconds) for expiry decisions.
#[must_use]
pub fn check_write_against_claims(
    targets: &[RepoPath],
    writer_agent_id: &StableId,
    claims: &[ClaimContract],
    now_unix: i64,
) -> WriteCheck {
    // Deterministic inspection order: sort a shallow copy of claim refs by id.
    // We never mutate the caller's slice.
    let mut ordered: Vec<&ClaimContract> = claims.iter().collect();
    ordered.sort_by(|a, b| a.id.0.cmp(&b.id.0));

    let mut blocks: Vec<BlockDetail> = Vec::new();
    let mut governed_by_self: Vec<RepoPath> = Vec::new();
    let mut ungoverned: Vec<RepoPath> = Vec::new();

    for target in targets {
        let target_norm = normalize_segments(&target.0);
        match classify_target(target, &target_norm, &ordered, writer_agent_id, now_unix) {
            TargetClass::Ungoverned => ungoverned.push(target.clone()),
            TargetClass::GovernedBySelf => governed_by_self.push(target.clone()),
            TargetClass::BlockedBy(detail) => blocks.push(detail),
        }
    }

    // Fail-closed: if ANY target collides, the whole write is blocked.
    if blocks.is_empty() {
        WriteCheck::Ok {
            governed_by_self,
            ungoverned,
        }
    } else {
        WriteCheck::Blocked { blocks }
    }
}

/// Internal per-target classification produced by [`classify_target`].
///
/// Each write target is independently sorted into one of three buckets.
/// The caller ([`check_write_against_claims`]) aggregates these into a single
/// [`WriteCheck`] using the fail-closed rule: any `BlockedBy` makes the whole
/// write [`WriteCheck::Blocked`], regardless of how many targets were clear.
#[derive(Debug)]
enum TargetClass {
    Ungoverned,
    GovernedBySelf,
    BlockedBy(BlockDetail),
}

/// Classify a single write target against all live claims on the bus.
///
/// Walks every claim in sorted order. For each *live* claim whose scope covers
/// the target path:
/// - If the holder is `writer_agent_id`, mark `seen_live_self = true` but keep
///   scanning — a *peer*'s live claim later in the iteration still wins (hard
///   block). This makes peer-trumps-self exact even when both agents hold
///   overlapping claims.
/// - If the holder is anyone else, return `BlockedBy` immediately — the first
///   peer collision is authoritative; we do not need to look further.
///
/// Targets that lexically climb above the repo root are classified as
/// `Ungoverned` before claim matching; otherwise, after all claims are scanned:
/// `seen_live_self` → `GovernedBySelf`; otherwise → `Ungoverned` (no live
/// claim covered it at all).
///
/// This is the heart of layer-2 prevention: it is what turns a semantic claim
/// ("alice owns scope X") into a hard write gate ("bob may NOT touch X").
fn classify_target(
    raw_target: &RepoPath,
    target_segments: &[String],
    claims: &[&ClaimContract],
    writer_agent_id: &StableId,
    now_unix: i64,
) -> TargetClass {
    if escapes_repo_root(target_segments) {
        return TargetClass::Ungoverned;
    }

    let mut seen_live_self = false;

    for claim in claims {
        // Only live claims govern writes (DD27).
        if !is_live(claim, now_unix) {
            continue;
        }
        // A claim with no paths governs no path (DD28).
        if !scope_covers_any(claim, target_segments) {
            continue;
        }

        let holder = &claim.claim.claimant_agent_id;
        if holder == writer_agent_id {
            // Own claim covers it — authorized. Keep scanning in case another
            // live claim by a peer also covers it (then it is still blocked).
            seen_live_self = true;
        } else {
            // Another agent's live claim covers this path — hard block.
            return TargetClass::BlockedBy(BlockDetail {
                blocked_path: raw_target.clone(),
                blocking_claim_id: claim.id.clone(),
                claimant: holder.clone(),
                conflict_code: ConflictCode::WriteTargetClaimed,
            });
        }
    }

    if seen_live_self {
        TargetClass::GovernedBySelf
    } else {
        TargetClass::Ungoverned
    }
}

/// Does any path in `claim.scope.paths` cover `target_segments`?
fn scope_covers_any(claim: &ClaimContract, target_segments: &[String]) -> bool {
    claim
        .scope
        .paths
        .iter()
        .any(|p| claim_path_covers(&p.0, target_segments))
}

/// True iff two repo paths overlap under the same segment-aware containment
/// rules used by write conflict detection.
///
/// This is deliberately about claim path vs claim path overlap, not about a
/// write target. Either path may be an exact file, a directory prefix, or an
/// explicit repo-root claim (`.`, `/`, `""`). A path that lexically escapes the
/// repo root (`..`) is treated as governing nothing.
#[must_use]
pub fn repo_paths_overlap(left: &RepoPath, right: &RepoPath) -> bool {
    let left_is_root = is_explicit_repo_root_path(&left.0);
    let right_is_root = is_explicit_repo_root_path(&right.0);

    if left_is_root && right_is_root {
        return true;
    }

    let left_segments = normalize_segments(&left.0);
    let right_segments = normalize_segments(&right.0);

    if escapes_repo_root(&left_segments) || escapes_repo_root(&right_segments) {
        return false;
    }

    if left_is_root {
        return !right_segments.is_empty();
    }
    if right_is_root {
        return !left_segments.is_empty();
    }

    path_covers(&left_segments, &right_segments) || path_covers(&right_segments, &left_segments)
}

/// True iff a raw claim path covers normalized target segments.
///
/// `normalize_segments` intentionally returns `[]` for both an empty target and
/// explicit repo-root spellings such as `.`, `/`, and ``. For claims, those
/// explicit root spellings are authority over the whole repository; for targets,
/// an empty path still covers nothing. Keeping the root decision on the claim
/// side preserves DD28 (a claim with no `scope.paths` governs no path) while
/// closing the false-negative where a root path governed nothing.
fn claim_path_covers(raw_claim_path: &str, target_segments: &[String]) -> bool {
    if target_segments.is_empty() {
        return false;
    }
    if is_explicit_repo_root_path(raw_claim_path) {
        return true;
    }
    path_covers(&normalize_segments(raw_claim_path), target_segments)
}

/// Is this claim path an explicit spelling of the repository root?
///
/// This deliberately accepts only empty, separator-only, or `.`-only spellings
/// (``, `.`, `/`, `./`, `\\`, etc.). It does not reinterpret arbitrary traversal
/// expressions as root, which keeps DD29's lexical traversal handling narrow.
fn is_explicit_repo_root_path(raw: &str) -> bool {
    raw.split(['/', '\\'])
        .all(|part| part.is_empty() || part == ".")
}

/// Does the normalized path lexically climb above the repository root?
fn escapes_repo_root(segments: &[String]) -> bool {
    segments.first().is_some_and(|segment| segment == "..")
}

/// True iff `claim_segments` contains `target_segments`.
///
/// Containment = exact match, OR `claim_segments` is a strict directory prefix
/// of `target_segments` (every segment matches and the target goes deeper).
/// This is segment-aware, defeating the `src/ev` vs `src/evil.rs` trap (DD26).
fn path_covers(claim_segments: &[String], target_segments: &[String]) -> bool {
    if claim_segments.is_empty() || target_segments.is_empty() {
        return false;
    }
    if claim_segments == target_segments {
        return true; // exact file match
    }
    // Directory containment: target must be strictly deeper than the claim dir.
    if target_segments.len() <= claim_segments.len() {
        return false;
    }
    claim_segments
        .iter()
        .zip(target_segments.iter())
        .all(|(c, t)| c == t)
}

/// Lexically normalize a posix-ish path into clean segments (DD29).
///
/// Drops `.` and collapses `..` lexically against preceding non-`..` segments
/// (no IO / no symlink resolution). Leading/excess `..` segments are preserved
/// so repo-root escapes do not normalize into governed paths. Both `/` and `\`
/// are treated as separators so Windows-style repo paths match. A leading
/// separator is ignored (all paths are repo-relative). An empty or root-only
/// path yields `[]`, which covers nothing and is covered by nothing.
fn normalize_segments(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => {
                // Collapse only if there is a non-`..` segment to pop; a
                // leading `..` (or excess `..`) is preserved as a segment so
                // `../../claimed` cannot become `["claimed"]`.
                if out.last().is_some_and(|s| s != "..") {
                    out.pop();
                } else {
                    out.push("..".to_string());
                }
            }
            // DD30: Unicode-lowercase so matching is case-INsensitive. This is
            // fail-closed for security: on a case-insensitive filesystem
            // (Windows NTFS, macOS APFS default, and the /mnt/c drvfs this
            // repo lives on) `Contracts/x` and `contracts/x` are the SAME
            // file; byte-exact matching would let a peer bypass a claim by
            // case-folding. Unicode lowercasing extends the same guard to
            // non-ASCII path segments (`Épicos` vs `épicos`). Lowercasing can
            // cause a *false block* on a purely case-sensitive filesystem where
            // `Foo.rs` and `foo.rs` are distinct — but a false block is safe
            // (the writer just acquires its own claim), whereas a missed block
            // is a collision.
            other => out.push(other.to_lowercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::claim::{
        ActorRole, ClaimIdentity, ClaimKind, ClaimLease, ClaimScope, ClaimScopeKind, ClaimStatus,
        ClaimStatusRecord, ExpiryAction, ExpiryPolicy, ReclaimPolicy,
    };
    use forge_core_contracts::common::{ClaimId, RepoPath, ScopeId, StableId};

    /// Build a live claim owned by `agent` over the given `paths`, expiring
    /// well after `now`. Keeps tests terse and readable.
    fn live_claim(id: &str, agent: &str, paths: &[&str]) -> ClaimContract {
        ClaimContract {
            id: ClaimId(id.to_string()),
            contract_ref: RepoPath("contracts/claims/x.yaml".to_string()),
            claim: ClaimIdentity {
                kind: ClaimKind::Story,
                claimant_agent_id: StableId(agent.to_string()),
                claimant_role: ActorRole::Worker,
                registry_ref: None,
            },
            scope: ClaimScope {
                kind: ClaimScopeKind::Story,
                id: ScopeId("S4.5".to_string()),
                product_area: None,
                paths: paths.iter().map(|p| RepoPath((*p).to_string())).collect(),
            },
            lease: ClaimLease {
                acquired_at: "2026-06-26T00:00:00Z".to_string(),
                last_heartbeat_at: "2026-06-26T00:00:00Z".to_string(),
                expires_at: "2099-01-01T00:00:00Z".to_string(),
                ttl_seconds: 600,
                heartbeat_interval_seconds: 120,
                expected_state_version: 1,
            },
            status: ClaimStatusRecord {
                value: ClaimStatus::Active,
                evaluated_at: "2026-06-26T00:00:00Z".to_string(),
                reason_code: None,
            },
            expiry_policy: ExpiryPolicy {
                on_expiry: ExpiryAction::RecordHandoffRequest,
                handoff_required: true,
                release_without_handoff_allowed: false,
                reclaim_policy: ReclaimPolicy::DriverReview,
                handoff_request_ref: None,
            },
            evidence_refs: Vec::new(),
        }
    }

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn exact_file_blocked_by_other_agent() {
        let claims = vec![live_claim("c1", "alice", &["contracts/stories/S4.5.yaml"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/stories/S4.5.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(check.is_blocked());
        match check {
            WriteCheck::Blocked { blocks } => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks[0].claimant.0, "alice");
                assert_eq!(blocks[0].conflict_code, ConflictCode::WriteTargetClaimed);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn same_agent_write_into_own_claim_is_authorized() {
        let claims = vec![live_claim("c1", "alice", &["contracts/stories/S4.5.yaml"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/stories/S4.5.yaml".to_string())],
            &StableId("alice".to_string()),
            &claims,
            NOW,
        );
        assert!(!check.is_blocked());
        match check {
            WriteCheck::Ok {
                governed_by_self,
                ungoverned,
            } => {
                assert_eq!(governed_by_self.len(), 1);
                assert!(ungoverned.is_empty());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn directory_claim_contains_nested_file() {
        let claims = vec![live_claim("c1", "alice", &["contracts/claims/"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/claims/x.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(
            check.is_blocked(),
            "nested file must be covered by dir claim"
        );
    }

    #[test]
    fn sibling_directory_is_not_containment() {
        // The classic trap: a claim on `src/ev` must NOT cover `src/evil.rs`.
        let claims = vec![live_claim("c1", "alice", &["src/ev"])];
        let check = check_write_against_claims(
            &[RepoPath("src/evil.rs".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(
            !check.is_blocked(),
            "segment-aware matching must reject src/ev => src/evil.rs"
        );
    }

    #[test]
    fn no_governing_claim_is_ungoverned_not_blocked() {
        let claims: Vec<ClaimContract> = vec![];
        let check = check_write_against_claims(
            &[RepoPath("docs/readme.md".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(!check.is_blocked());
        match check {
            WriteCheck::Ok {
                ungoverned,
                governed_by_self,
            } => {
                assert_eq!(ungoverned.len(), 1);
                assert!(governed_by_self.is_empty());
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn expired_claim_does_not_block() {
        let mut claim = live_claim("c1", "alice", &["contracts/stories/S4.5.yaml"]);
        // Set expiry in the past relative to NOW.
        claim.lease.expires_at = "2000-01-01T00:00:00Z".to_string();
        let check = check_write_against_claims(
            &[RepoPath("contracts/stories/S4.5.yaml".to_string())],
            &StableId("bob".to_string()),
            &[claim],
            NOW,
        );
        assert!(
            !check.is_blocked(),
            "expired claim has no active owner — must not block"
        );
    }

    #[test]
    fn path_traversal_does_not_escape_claim() {
        let claims = vec![live_claim("c1", "alice", &["contracts/claims/"])];
        // `contracts/claims/../secret` normalizes to `contracts/secret`, which
        // is OUTSIDE the claim — must NOT be blocked (it is not actually inside
        // the dir).
        let check = check_write_against_claims(
            &[RepoPath("contracts/claims/../secret".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(
            !check.is_blocked(),
            "lexical normalization must prevent traversal-based false block"
        );
    }

    #[test]
    fn leading_parent_segments_do_not_normalize_into_governed_path() {
        let claims = vec![live_claim("c1", "alice", &["claimed"])];
        let raw = "../../claimed";

        assert_eq!(
            normalize_segments(raw),
            vec!["..".to_string(), "..".to_string(), "claimed".to_string()]
        );

        let check = check_write_against_claims(
            &[RepoPath(raw.to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );

        assert_ungoverned_escape(check, raw);

        let root_claims = vec![live_claim("c-root", "alice", &["."])];
        let root_check = check_write_against_claims(
            &[RepoPath(raw.to_string())],
            &StableId("bob".to_string()),
            &root_claims,
            NOW,
        );
        assert_ungoverned_escape(root_check, raw);
    }

    fn assert_ungoverned_escape(check: WriteCheck, raw: &str) {
        match check {
            WriteCheck::Ok {
                governed_by_self,
                ungoverned,
            } => {
                assert!(governed_by_self.is_empty());
                assert_eq!(ungoverned, vec![RepoPath(raw.to_string())]);
            }
            WriteCheck::Blocked { blocks } => {
                panic!("escaping target must not be blocked as governed: {blocks:?}");
            }
        }
    }

    #[test]
    fn explicit_repo_root_claim_blocks_descendant_paths() {
        for root_path in [".", "/", ""] {
            let claims = vec![live_claim("c1", "alice", &[root_path])];
            let check = check_write_against_claims(
                &[RepoPath("src/lib.rs".to_string())],
                &StableId("bob".to_string()),
                &claims,
                NOW,
            );
            assert!(
                check.is_blocked(),
                "root claim path `{root_path}` must cover repository descendants"
            );
        }
    }

    #[test]
    fn empty_paths_claim_blocks_nothing() {
        let claim = live_claim("c1", "alice", &[]); // no paths
        let check = check_write_against_claims(
            &[RepoPath("anything/here.yaml".to_string())],
            &StableId("bob".to_string()),
            &[claim],
            NOW,
        );
        assert!(!check.is_blocked());
    }

    #[test]
    fn multi_target_partial_block_blocks_whole_write() {
        let claims = vec![live_claim("c1", "alice", &["contracts/stories/S4.5.yaml"])];
        let targets = vec![
            RepoPath("docs/free.md".to_string()), // ungoverned
            RepoPath("contracts/stories/S4.5.yaml".to_string()), // blocked
        ];
        let check =
            check_write_against_claims(&targets, &StableId("bob".to_string()), &claims, NOW);
        // Fail-closed: one collision blocks the whole write set.
        assert!(check.is_blocked());
    }

    #[test]
    fn peer_claim_trumps_own_claim_for_same_path() {
        // If BOTH alice and bob have live claims on the same path, bob writing
        // is still blocked — a peer's live claim is a hard block regardless of
        // bob's own coverage. (In practice acquire prevents two claims on one
        // scope; this guards the matching logic itself.)
        let claims = vec![
            live_claim("c1", "alice", &["contracts/stories/S4.5.yaml"]),
            live_claim("c2", "bob", &["contracts/stories/S4.5.yaml"]),
        ];
        let check = check_write_against_claims(
            &[RepoPath("contracts/stories/S4.5.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(check.is_blocked());
    }

    #[test]
    fn determinism_blocks_reported_in_sorted_claim_order() {
        // Reverse the claim input order; the single reported block must still
        // attribute to the lexicographically-first claim id.
        let claims = vec![
            live_claim("zzz", "alice", &["a/x.yaml"]),
            live_claim("aaa", "carol", &["a/x.yaml"]),
        ];
        let check = check_write_against_claims(
            &[RepoPath("a/x.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        match check {
            WriteCheck::Blocked { blocks } => {
                assert_eq!(blocks[0].blocking_claim_id.0, "aaa");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn windows_style_separators_supported() {
        let claims = vec![live_claim("c1", "alice", &["contracts\\claims"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/claims/x.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(check.is_blocked(), "mixed separators must still match");
    }

    #[test]
    fn empty_target_is_never_blocked() {
        let claims = vec![live_claim("c1", "alice", &["contracts/claims/"])];
        let check = check_write_against_claims(
            &[RepoPath(String::new())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(
            !check.is_blocked(),
            "empty path covers nothing and is covered by nothing"
        );
    }

    #[test]
    fn claim_directory_without_trailing_slash_still_contains() {
        // `contracts/claims` (no trailing slash) must still contain
        // `contracts/claims/x.yaml` — trailing slash is cosmetic, not semantic.
        let claims = vec![live_claim("c1", "alice", &["contracts/claims"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/claims/x.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(check.is_blocked());
    }

    #[test]
    fn case_insensitive_matching_blocks_case_folded_peer() {
        // DD30: on a case-insensitive filesystem (this repo's /mnt/c),
        // `Contracts/x` and `contracts/x` are the SAME file. Alice claims
        // one casing; Bob must be blocked on the other casing.
        let claims = vec![live_claim("c1", "alice", &["Contracts/Stories/S5.0.yaml"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/stories/s5.0.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(
            check.is_blocked(),
            "case-folded path must collide with differently-cased claim"
        );
    }

    #[test]
    fn unicode_case_insensitive_matching_blocks_case_folded_peer() {
        let claims = vec![live_claim("c1", "alice", &["Contracts/Épicos/S5.0.yaml"])];
        let check = check_write_against_claims(
            &[RepoPath("contracts/épicos/s5.0.yaml".to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        assert!(
            check.is_blocked(),
            "Unicode case-folded path must collide with differently-cased claim"
        );
    }

    #[test]
    fn blocked_path_reports_raw_input_not_normalized() {
        // M2: the block detail must echo the EXACT target the writer
        // submitted, so it can correlate the block back to its request — even
        // though matching used the normalized form internally.
        let claims = vec![live_claim("c1", "alice", &["contracts/stories/S5.0.yaml"])];
        let raw = "a/../contracts/stories/S5.0.yaml";
        let check = check_write_against_claims(
            &[RepoPath(raw.to_string())],
            &StableId("bob".to_string()),
            &claims,
            NOW,
        );
        match check {
            WriteCheck::Blocked { blocks } => {
                assert_eq!(blocks[0].blocked_path.0, raw);
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn repo_paths_overlap_uses_same_segment_containment_rules_as_write_checks() {
        assert!(repo_paths_overlap(
            &RepoPath("contracts/claims".to_string()),
            &RepoPath("contracts/claims/x.yaml".to_string())
        ));
        assert!(repo_paths_overlap(
            &RepoPath("Contracts/Stories/S5.0.yaml".to_string()),
            &RepoPath("contracts/stories/s5.0.yaml".to_string())
        ));
        assert!(repo_paths_overlap(
            &RepoPath(".".to_string()),
            &RepoPath("src/lib.rs".to_string())
        ));
        assert!(!repo_paths_overlap(
            &RepoPath("src/ev".to_string()),
            &RepoPath("src/evil.rs".to_string())
        ));
        assert!(!repo_paths_overlap(
            &RepoPath("../../claimed".to_string()),
            &RepoPath("claimed".to_string())
        ));
    }
}
