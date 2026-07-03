//! F14 Knowledge Orchestration — the research source trust model (ADR-0010).
//!
//! Candidato 1 (pure decision functions): [`ResearchContract::can_admit_source`]
//! decides whether a [`ResearchSource`] may ENTER the Source Ledger, under a
//! [`ResearchPolicy`]. This module is the PDP (Policy Decision Point); the PEP
//! that enforces it under an exclusive file lock lives in `forge-core-research`
//! (mirrors the F06 `MemoryContract`/`forge-core-memory` split — ADR-0024).
//!
//! # Trust boundary (ADR-0010, decision §5)
//!
//! The F14 admission gate is fail-closed and attests to **resolution**, not to
//! quality/tier. A `ResearchSource` is admitted when it satisfies the policy
//! (permitted kind, required provenance fields, optional content-hash
//! requirement). Admitting a source never attests to its truthfulness — only
//! that it is registered and reachable for citation. Tier-minimum gates are a
//! future, separate axis (analogous to how F06 separates authority from
//! review). See `CONTEXT.md` "Knowledge Orchestration (F14)".
//!
//! # Distinct from `EvidenceSource` (ADR-0010, decision §1)
//!
//! [`EvidenceSource`](crate::EvidenceSource) (in `evidence.rs`) is the
//! **curated**, static, tier-graded source that backs design decisions of the
//! Forge itself (`FieldEvidenceRegistry`). [`ResearchSource`] is the
//! **runtime-harvested** source a research agent collects during a run. They
//! are distinct populations of trust; they share only the [`SourceId`]
//! namespace (the reuse boundary), never a struct. Fusing them would
//! re-introduce the Model B class of bug one layer down (ADR-0010 rationale).

use crate::common::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Source kind
// -----------------------------------------------------------------------------

/// The semantic class of a [`ResearchSource`]. Sharper than a free-form string:
/// a typo here is a compile error, and the policy's `permitted_source_kinds`
/// membership check is total (matches the F06 `MemoryKind` discipline).
///
/// Variants are the kinds a research agent realistically harvests at runtime:
/// papers, web pages, local documents, and repository references. Extensible by
/// adding variants; the policy's allow-list is the gate, not the enum's
/// exhaustiveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResearchSourceKind {
    /// A pre-print or published paper (arXiv, DOI, conference proceedings).
    Paper,
    /// A web page or online resource fetched at runtime.
    WebUrl,
    /// A file on the local filesystem (a doc, a log, a dump).
    LocalDoc,
    /// A reference to a repository (commit, tag, path).
    RepoRef,
}

// -----------------------------------------------------------------------------
// Source + Policy + Decision
// -----------------------------------------------------------------------------

/// A source harvested at runtime by an agent during a run, registered in the
/// Source Ledger with provenance. The unit of citation backing on the runtime
/// side (ADR-0010). Distinct from [`EvidenceSource`](crate::EvidenceSource),
/// which is the curated/field-evidence kind — see the module doc.
///
/// `content_hash` and `trace_ref` are optional so a policy can choose to
/// require or omit them (`require_content_hash`, etc.); the gate enforces
/// whatever the policy demands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResearchSource {
    /// Project-wide-unique id. Shared namespace with `EvidenceSource.id`
    /// (the reuse boundary — ADR-0010 §1). A collision across the two backings
    /// is a registration error caught by the citation check, not here.
    pub id: SourceId,
    /// The semantic class; gated by `ResearchPolicy::permitted_source_kinds`.
    pub kind: ResearchSourceKind,
    /// Human-readable title (non-empty required at the gate).
    pub title: String,
    /// How to reach the source: a URL, a DOI, a filesystem path, a repo ref.
    /// Free-form string by design (the locator shape varies per kind); the
    /// gate requires it non-empty.
    pub locator: String,
    /// When the agent fetched/observed the source (UNIX seconds). Provenance.
    pub fetched_at: u64,
    /// Optional content integrity hash (`"sha256:..."`). Required iff the
    /// policy sets `require_content_hash: true`. Drift detection (hash vs
    /// current content) is a future story — MVP attests only to presence.
    #[serde(default)]
    pub content_hash: Option<String>,
    /// The principal (agent or human) that harvested the source. Provenance.
    pub harvested_by: String,
    /// Optional reference to the run/trace that produced the harvest. Required
    /// iff the policy sets `require_trace_ref: true`.
    #[serde(default)]
    pub trace_ref: Option<String>,
}

/// The F14 admission policy. Parametric (YAML), mirrors `MemoryPolicy`: a
/// single typed object, not scattered primitives. The "research mode" **is**
/// the active policy — there is no `research run` pipeline (G1
/// anti-script-de-novela; ADR-0010 §6).
///
/// **No `Default`** on purpose (same guardrail as `MemoryPolicy`): a
/// permissive default would silently admit sources the operator never
/// sanctioned; a deny-all default would be correct but surprising if silently
/// active. Callers must construct a policy explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResearchPolicy {
    /// Source kinds permitted to enter the ledger. Empty = deny all
    /// (fail-closed). Order-irrelevant membership check.
    pub permitted_source_kinds: Vec<ResearchSourceKind>,
    /// When `true`, a source without a non-empty `content_hash` is rejected.
    pub require_content_hash: bool,
    /// When `true`, a source without a non-empty `trace_ref` is rejected.
    pub require_trace_ref: bool,
}

/// A trust-gate decision for research source admission (PDP return). Pure:
/// carries no authority, mutates nothing. Fail-closed: `Blocked` is `Blocked`
/// even with an empty reason list (mirrors `AdmissionDecision`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[must_use]
pub enum ResearchAdmissionDecision {
    Allowed,
    Blocked(Vec<ResearchAdmissionDenialReason>),
}

impl ResearchAdmissionDecision {
    #[inline]
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    #[inline]
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    /// Build the decision from denials: empty ⇒ Allowed, else Blocked(reasons).
    /// Centralised so the fail-closed rule lives in one place (a future caller
    /// cannot invert it). Mirrors `AdmissionDecision::from_denials`.
    pub fn from_denials(denials: Vec<ResearchAdmissionDenialReason>) -> Self {
        if denials.is_empty() {
            Self::Allowed
        } else {
            Self::Blocked(denials)
        }
    }
}

/// Why a research source admission was blocked. As-data (Cedar-style
/// diagnostics), one named reason per gate arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResearchAdmissionDenialReason {
    /// `kind` not in the policy's `permitted_source_kinds` (includes the
    /// deny-all empty-list default).
    KindNotPermitted,
    /// `title` empty or whitespace-only.
    MissingTitle,
    /// `locator` empty or whitespace-only.
    MissingLocator,
    /// Policy required `content_hash` and the source had none / empty.
    MissingContentHash,
    /// Policy required `trace_ref` and the source had none / empty.
    MissingTraceRef,
}

// -----------------------------------------------------------------------------
// PDP
// -----------------------------------------------------------------------------

/// Pure decision functions for the F14 trust model (the PDP). The PEP in
/// `forge-core-research` calls these and performs the mutation under a lock.
impl ResearchContract {
    /// F14 admission gate. Decides whether `source` may ENTER the Source
    /// Ledger. Fail-closed: any missing policy requirement blocks. Admitting
    /// attests to **registration/resolution**, never to truthfulness or tier
    /// (ADR-0010 §5). See `CONTEXT.md` "Citation Check".
    pub fn can_admit_source(
        source: &ResearchSource,
        policy: &ResearchPolicy,
    ) -> ResearchAdmissionDecision {
        let mut denials = Vec::new();

        if !kind_is_permitted(source.kind, &policy.permitted_source_kinds) {
            denials.push(ResearchAdmissionDenialReason::KindNotPermitted);
        }
        if source.title.trim().is_empty() {
            denials.push(ResearchAdmissionDenialReason::MissingTitle);
        }
        if source.locator.trim().is_empty() {
            denials.push(ResearchAdmissionDenialReason::MissingLocator);
        }
        if policy.require_content_hash
            && source
                .content_hash
                .as_deref()
                .is_none_or(|h| h.trim().is_empty())
        {
            denials.push(ResearchAdmissionDenialReason::MissingContentHash);
        }
        if policy.require_trace_ref
            && source
                .trace_ref
                .as_deref()
                .is_none_or(|t| t.trim().is_empty())
        {
            denials.push(ResearchAdmissionDenialReason::MissingTraceRef);
        }

        ResearchAdmissionDecision::from_denials(denials)
    }
}

/// Zero-variant marker type carrying the F14 PDP (pure decision functions).
///
/// Following the F06 precedent, the decisions are associated functions on an
/// `impl` block rather than free functions, so the `ResearchContract::name`
/// call site reads identically to `MemoryContract::can_admit`. The type itself
/// is never instantiated (it has no variants); it exists only to namespace the
/// pure API. This keeps the contract surface auditable in one place and makes
/// the PDP/PEP split visually obvious at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchContract {}

/// Membership check: is `kind` in `permitted`? Order-irrelevant (Cedar
/// order-independence-of-evaluation), `Vec`-based to honour the crate's "derive
/// `Eq`, never `Ord`/`Hash`" convention. Mirrors the F06 `kind_is_permitted`.
#[must_use]
fn kind_is_permitted(kind: ResearchSourceKind, permitted: &[ResearchSourceKind]) -> bool {
    permitted.contains(&kind)
}

// -----------------------------------------------------------------------------
// Tests — pure PDP only. PEP/storage tests live in forge-core-research.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::SourceId;

    fn sample_source(id: &str, kind: ResearchSourceKind) -> ResearchSource {
        ResearchSource {
            id: SourceId(id.into()),
            kind,
            title: "A canonical source".into(),
            locator: "https://example.org/source".into(),
            fetched_at: 1_700_000_000,
            content_hash: Some("sha256:abc".into()),
            harvested_by: "agent.1".into(),
            trace_ref: Some("run.1".into()),
        }
    }

    fn permissive_policy() -> ResearchPolicy {
        ResearchPolicy {
            permitted_source_kinds: vec![
                ResearchSourceKind::Paper,
                ResearchSourceKind::WebUrl,
                ResearchSourceKind::LocalDoc,
                ResearchSourceKind::RepoRef,
            ],
            require_content_hash: false,
            require_trace_ref: false,
        }
    }

    fn deny_all_policy() -> ResearchPolicy {
        ResearchPolicy {
            permitted_source_kinds: vec![],
            require_content_hash: false,
            require_trace_ref: false,
        }
    }

    #[test]
    fn permissive_policy_admits_well_formed_source() {
        let decision = ResearchContract::can_admit_source(
            &sample_source("s.one", ResearchSourceKind::Paper),
            &permissive_policy(),
        );
        assert!(decision.is_allowed());
    }

    #[test]
    fn deny_all_policy_blocks_every_kind() {
        for kind in [
            ResearchSourceKind::Paper,
            ResearchSourceKind::WebUrl,
            ResearchSourceKind::LocalDoc,
            ResearchSourceKind::RepoRef,
        ] {
            let decision =
                ResearchContract::can_admit_source(&sample_source("s.x", kind), &deny_all_policy());
            assert!(decision.is_blocked(), "{kind:?} should be denied");
        }
    }

    #[test]
    fn kind_not_in_permitted_subset_is_blocked() {
        let policy = ResearchPolicy {
            permitted_source_kinds: vec![ResearchSourceKind::Paper],
            require_content_hash: false,
            require_trace_ref: false,
        };
        let decision = ResearchContract::can_admit_source(
            &sample_source("s.web", ResearchSourceKind::WebUrl),
            &policy,
        );
        assert!(decision.is_blocked());
    }

    #[test]
    fn empty_title_is_blocked() {
        let mut s = sample_source("s.title", ResearchSourceKind::Paper);
        s.title = "   ".into();
        let decision = ResearchContract::can_admit_source(&s, &permissive_policy());
        assert!(decision.is_blocked());
    }

    #[test]
    fn empty_locator_is_blocked() {
        let mut s = sample_source("s.loc", ResearchSourceKind::Paper);
        s.locator = String::new();
        let decision = ResearchContract::can_admit_source(&s, &permissive_policy());
        assert!(decision.is_blocked());
    }

    #[test]
    fn required_content_hash_absent_is_blocked() {
        let policy = ResearchPolicy {
            permitted_source_kinds: vec![ResearchSourceKind::Paper],
            require_content_hash: true,
            require_trace_ref: false,
        };
        let mut s = sample_source("s.nohash", ResearchSourceKind::Paper);
        s.content_hash = None;
        let decision = ResearchContract::can_admit_source(&s, &policy);
        assert!(decision.is_blocked());
    }

    #[test]
    fn required_content_hash_empty_is_blocked() {
        let policy = ResearchPolicy {
            permitted_source_kinds: vec![ResearchSourceKind::Paper],
            require_content_hash: true,
            require_trace_ref: false,
        };
        let mut s = sample_source("s.emptyhash", ResearchSourceKind::Paper);
        s.content_hash = Some("   ".into());
        let decision = ResearchContract::can_admit_source(&s, &policy);
        assert!(decision.is_blocked());
    }

    #[test]
    fn required_trace_ref_absent_is_blocked() {
        let policy = ResearchPolicy {
            permitted_source_kinds: vec![ResearchSourceKind::Paper],
            require_content_hash: false,
            require_trace_ref: true,
        };
        let mut s = sample_source("s.notrace", ResearchSourceKind::Paper);
        s.trace_ref = None;
        let decision = ResearchContract::can_admit_source(&s, &policy);
        assert!(decision.is_blocked());
    }

    #[test]
    fn from_denials_empty_is_allowed() {
        assert!(ResearchAdmissionDecision::from_denials(vec![]).is_allowed());
    }

    #[test]
    fn from_denials_nonempty_is_blocked() {
        let d = ResearchAdmissionDecision::from_denials(vec![
            ResearchAdmissionDenialReason::MissingTitle,
        ]);
        assert!(d.is_blocked());
    }

    #[test]
    fn kind_is_permitted_is_order_independent() {
        let permitted = [
            ResearchSourceKind::WebUrl,
            ResearchSourceKind::Paper,
            ResearchSourceKind::LocalDoc,
        ];
        assert!(kind_is_permitted(ResearchSourceKind::Paper, &permitted));
        assert!(!kind_is_permitted(ResearchSourceKind::RepoRef, &permitted));
    }
}
