//! F07 — Multi-principal governance contracts.
//!
//! Three-layer model (ADR-0007, Accepted): authorization (who) + coordination
//! (declared intents) + conflict (first-class objects, never silent merge).
//!
//! - [`GovernancePolicy`] — who may operate / attest reviews (ReBAC/Cedar-shaped
//!   authorization layer). This is the PDP input; it answers "may principal P
//!   act?", the single-principal question. It does NOT detect A↔B conflict.
//! - [`IntentContract`] — a principal's declared intent over an authority scope
//!   with an expiry. The Gray-style intent-lock analog (coordination layer).
//!   `expires_at` is load-bearing (liveness/correctness per Spanner/2PL).
//! - [`ConflictContract`] — a first-class conflict object emitted when two
//!   intents overlap (Git/Apel/Berenson lineage). F07's whole point: conflict
//!   becomes a named, typed, halting object that requires explicit resolution,
//!   NOT a silent merge (the CRDT/OT/XACML posture, which destroys the signal).
//!
//! No agent-governance standard yet covers resource contention (`MAST` `NeurIPS`
//! 2025, `GaaS` arXiv:2508.18765 are 2025–26, still forming) — this is
//! research-grounded rather than standards-compliant.

use crate::common::{PrincipalId, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The scope a principal declares intent over (the "locked sub-tree"). Mirrors
/// the [`crate::MemoryScope`] shape: a kind + a target. Conflict detection
/// compares two `IntentScope`s for overlap (the lock-compatibility matrix).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentScope {
    pub kind: IntentScopeKind,
    /// The scoped resource: a repo path, a memory entry id, a project id —
    /// depending on `kind`. A `StableId` (a resource id), deliberately NOT a
    /// `PrincipalId` (R8: distinct concepts, distinct types).
    pub target: StableId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentScopeKind {
    /// The whole project.
    Project,
    /// A single repo.
    Repo,
    /// A path-prefix sub-tree (the canonical case — matches the existing
    /// path-segment claim-overlap detector at
    /// `forge-core-decisions/src/conflict_detection.rs`).
    PathPrefix,
    /// A single memory entry (for the capability-governance gap that unblocks
    /// F06's `review` axis).
    MemoryEntry,
}

/// A principal's declared intent — the Gray-style intent-lock analog. Declared
/// at claim-acquire time; two overlapping intents yield a [`ConflictContract`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntentContract {
    /// Resource id of this intent record itself.
    pub intent_id: StableId,
    /// WHO declares the intent (R8-distinct from resource ids).
    pub principal: PrincipalId,
    /// What the principal intends to accomplish (free text, for auditability).
    pub goal: String,
    /// The authority sub-tree the intent locks.
    pub authority_scope: IntentScope,
    /// LOAD-BEARING lease bound (unix seconds). An intent without an expiry is
    /// a permanent lock → deadlock/liveness failure (Gray 2PL, Spanner
    /// commit-wait). The validator rejects an already-expired intent.
    pub expires_at: u64,
    /// When the intent was declared (unix seconds). Must be < `expires_at`.
    pub declared_at: u64,
}

/// A first-class conflict object, EMITTED (not derived) when two intents
/// overlap. The F07 NFR: conflict becomes a structured object, never a silent
/// merge. Carries the attribution data the claim-acquire engine already
/// computes (`principal_a`/`principal_b`, `intent_a`/`intent_b`, the contested
/// scope) so F07.4 can populate it from `claim_engine.rs:317`'s existing
/// `PathAlreadyClaimed` rejection without inventing new fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConflictContract {
    /// Resource id of this conflict record.
    pub conflict_id: StableId,
    /// The two conflicting intents (refs to [`IntentContract::intent_id`]).
    pub intent_a: StableId,
    pub intent_b: StableId,
    /// The two conflicting principals (denormalized from the intents for
    /// queryability without a join — the arbitration ledger is append-only).
    pub principal_a: PrincipalId,
    pub principal_b: PrincipalId,
    /// The overlapping authority sub-tree that triggered the conflict.
    pub contested_scope: IntentScope,
    /// Why the conflict was detected (named, like a Berenson anomaly).
    pub detection_reason: ConflictDetectionReason,
    /// When the conflict was detected (unix seconds).
    pub detected_at: u64,
    /// The resolution lifecycle: Pending → Resolved/Escalated.
    pub resolution: ConflictResolutionState,
}

/// Named conflict reasons (Berenson-anomaly lineage: conflicts are named
/// phenomena, not opaque failures).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictDetectionReason {
    /// Two intents lock overlapping path prefixes (the canonical case, matches
    /// `repo_paths_overlap` in `conflict_detection.rs`).
    PathOverlap,
    /// Two intents lock the same authority scope at a coarser granularity
    /// (e.g. both Project-scoped).
    AuthorityScopeOverlap,
    /// Two principals mutated the same resource concurrently (detected at
    /// exercise time, not declaration time).
    ConcurrentMutate,
}

/// The resolution lifecycle of a [`ConflictContract`]. `Pending` is the
/// default on emission; an arbiter (a principal permitted by the
/// [`GovernancePolicy`]) moves it to `Resolved` or `Escalated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionState {
    /// Detected, awaiting arbitration.
    Pending,
    /// An arbiter decided. `arbiter` is a [`PrincipalId`] permitted to resolve
    /// (authorized by the [`GovernancePolicy`]).
    Resolved {
        arbiter: PrincipalId,
        decided_at: u64,
        decision: ResolutionDecision,
    },
    /// Escalated to a human/external arbiter (no authorized in-system arbiter).
    Escalated,
}

/// How a resolved conflict was decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionDecision {
    /// The contested scope was awarded to one principal.
    AwardedTo(PrincipalId),
    /// Both intents released; the scope is open for a fresh declaration.
    BothReleased,
    /// The scope split between the two principals (e.g. along a finer
    /// path-prefix boundary).
    SplitScope,
}

/// Who may operate and who may attest reviews (the authorization layer).
/// ReBAC/Cedar-shaped: it models *who a principal is* and *what they may do*,
/// answering the single-principal question. It does NOT detect A↔B conflict
/// (that is the [`ConflictContract`] layer's job).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GovernancePolicy {
    /// Resource id of this policy.
    pub policy_id: StableId,
    /// Principals permitted to operate at all (declare intents). Empty = deny
    /// all (fail-closed, matching the F06 `MemoryPolicy` convention).
    pub permitted_principals: Vec<PrincipalId>,
    /// Principals authorized to attest memory reviews (unblocks F06's deferred
    /// `memory review` verb — ADR 0002's reviewer-authorization requirement).
    pub authorized_reviewers: Vec<PrincipalId>,
    /// How conflicts are handled. `EmitContract` is the F07 default and the
    /// only correct posture; `SilentLastWriterWins` is present for completeness
    /// and the validator warns on it (the documented anti-pattern).
    pub conflict_policy: ConflictPolicy,
}

/// The conflict-handling posture. `SilentLastWriterWins` is the anti-pattern
/// F07 exists to forbid (CRDT/XACML posture that destroys the conflict signal);
/// the validator emits a warning diagnostic on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Emit a [`ConflictContract`] on overlap (the F07 default; structured
    /// conflict, never silent merge).
    EmitContract,
    /// Last writer wins silently (the anti-pattern; warns under validation).
    SilentLastWriterWins,
}

impl Default for ConflictPolicy {
    /// The default is `EmitContract` — F07's entire purpose. A
    /// `SilentLastWriterWins` default would re-introduce the silent-merge
    /// behavior F07 exists to forbid.
    fn default() -> Self {
        Self::EmitContract
    }
}

impl GovernancePolicy {
    /// The pure authorization PDP for arbitration: may `principal` move a
    /// [`ConflictContract`] out of [`Pending`](ConflictResolutionState::Pending)?
    ///
    /// This is the single-principal question ("is P an authorized arbiter?"),
    /// parallel in shape to [`crate::MemoryContract::can_admit`] (the F06 PDP):
    /// a pure predicate the PEP consults under the write lock (ADR-0007 layer 1
    /// — authorization — answering "may P act?"). It does NOT decide the
    /// *merit* of the resolution (that is `ResolutionDecision`); it authorizes
    /// the *actor*.
    ///
    /// Today this reuses [`Self::authorized_reviewers`] as the arbiter set: the
    /// reviewer role and the arbiter role coincide (a reviewer is trusted to
    /// adjudicate conflicts over what they can already attest). When the two
    /// powers diverge — a principal may review memory but not arbitrate, or
    /// vice-versa — this becomes a distinct `authorized_arbiters` field and a
    /// reviewer↔arbiter migration; that refinement is deferred until a policy
    /// actually needs it (YAGNI; no fixture today separates them).
    ///
    /// Fail-closed: an empty `authorized_reviewers` denies everyone, matching
    /// the F06 `MemoryPolicy` empty-`permitted_kinds` convention and the
    /// ReBAC/Cedar default-deny posture.
    #[must_use]
    pub fn can_arbitrate(&self, principal: &PrincipalId) -> bool {
        self.authorized_reviewers.iter().any(|p| p == principal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn governance_policy_fixture_round_trips() {
        let yaml = include_str!("../../../contracts/examples/governance-policy.yaml");
        let policy: GovernancePolicy = yaml_serde::from_str(yaml).expect("parse policy fixture");
        assert_eq!(policy.policy_id.0, "governance.policy.forge");
        assert_eq!(policy.permitted_principals.len(), 2);
        assert_eq!(
            policy.permitted_principals[0],
            PrincipalId("principal.alice".into())
        );
        assert_eq!(
            policy.authorized_reviewers[0],
            PrincipalId("principal.daniel".into())
        );
        assert_eq!(policy.conflict_policy, ConflictPolicy::EmitContract);
        // Round-trip.
        let serialized = yaml_serde::to_string(&policy).expect("serialize policy");
        let reparsed: GovernancePolicy = yaml_serde::from_str(&serialized).expect("reparse policy");
        assert_eq!(policy, reparsed);
    }

    #[test]
    fn intent_contract_fixture_round_trips() {
        let yaml = include_str!("../../../contracts/examples/intent-contract.yaml");
        let intent: IntentContract = yaml_serde::from_str(yaml).expect("parse intent fixture");
        assert_eq!(intent.intent_id.0, "intent.alice.f07");
        assert_eq!(intent.principal, PrincipalId("principal.alice".into()));
        assert_eq!(intent.authority_scope.kind, IntentScopeKind::PathPrefix);
        assert_eq!(intent.expires_at, 1_780_010_000);
        assert!(intent.expires_at > intent.declared_at);
        // Round-trip.
        let serialized = yaml_serde::to_string(&intent).expect("serialize intent");
        let reparsed: IntentContract = yaml_serde::from_str(&serialized).expect("reparse intent");
        assert_eq!(intent, reparsed);
    }

    #[test]
    fn conflict_contract_fixture_round_trips() {
        let yaml = include_str!("../../../contracts/examples/conflict-contract.yaml");
        let conflict: ConflictContract =
            yaml_serde::from_str(yaml).expect("parse conflict fixture");
        assert_eq!(conflict.conflict_id.0, "conflict.alice-bob.stories");
        assert_eq!(conflict.principal_a, PrincipalId("principal.alice".into()));
        assert_eq!(conflict.principal_b, PrincipalId("principal.bob".into()));
        assert_ne!(conflict.principal_a, conflict.principal_b);
        assert_eq!(
            conflict.detection_reason,
            ConflictDetectionReason::PathOverlap
        );
        assert_eq!(conflict.resolution, ConflictResolutionState::Pending);
        // Round-trip.
        let serialized = yaml_serde::to_string(&conflict).expect("serialize conflict");
        let reparsed: ConflictContract =
            yaml_serde::from_str(&serialized).expect("reparse conflict");
        assert_eq!(conflict, reparsed);
    }

    #[test]
    fn principal_id_is_serde_transparent_and_comparable() {
        // serde-transparent: serializes as a bare string, not an object.
        let value = serde_json::to_value(PrincipalId("principal.test".into())).expect("serialize");
        assert!(
            value.as_str().is_some(),
            "PrincipalId must serialize as a bare string: {value}"
        );
        assert_eq!(value.as_str(), Some("principal.test"));
        // R8: distinct from StableId at the type level (this is the compile-time
        // guarantee — equality across the two types would not compile).
        let a = PrincipalId("p".into());
        let b = PrincipalId("p".into());
        assert_eq!(a, b);
        let c = PrincipalId("q".into());
        assert_ne!(a, c);
    }
}
