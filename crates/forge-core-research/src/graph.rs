//! F14 Evidence Graph — the derived projection `SourceId → citing claims`
//! (ADR-0010 §"Consequencias": *"`EvidenceGraph` nao e tipo first-class nem
//! populado pelo agent: e projecao `SourceId` -> claims citantes, computada por
//! walk sobre artifacts (mesmo padrao do `reference_index` do forge-core-store)"*).
//!
//! # Why a projection, not a stored type
//!
//! A research "claim" is polymorphic — any node that carries a `source_id`
//! (ADR-0010). The Evidence Graph therefore cannot be populated by the agent
//! (who would have to know every claim shape); it is **derived** by walking the
//! same parsed YAML artifacts the validator already collects. This mirrors
//! `forge-core-store`'s `build_reference_index`: an index rebuilt on demand,
//! never the source of truth, disposable and deterministic. Storing it would
//! duplicate information already present in the artifacts and create a
//! consistency problem (the index could drift from the artifacts).
//!
//! # `ClaimRef`
//!
//! A [`ClaimRef`] is the stable address of one `source_id` occurrence: the
//! artifact path plus the cited id. It is coarse by design — the source id is
//! the grouping key, and the artifact path is the audit address. A finer
//! JSON-pointer locator can be layered on later if a caller needs sub-document
//! precision; the MVP needs "which artifact cites which source", which this
//! answers.
//!
//! # Determinism
//!
//! The output is a [`BTreeMap`] keyed by `SourceId.0`, with each value a
//! `Vec<ClaimRef>` sorted by `(document_path, source_id)`. Feeding the same
//! artifacts always yields byte-identical output (the Fowler replay guarantee
//! the projection itself upholds), so the graph is diffable across runs and
//! safe to assert on in tests.

use std::collections::BTreeMap;

use forge_core_validate::ParsedYamlDocument;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use yaml_serde::Value;

use crate::ResearchProjection;

/// The stable address of one citation: an artifact that carries a `source_id`.
/// The grouping key of the Evidence Graph on the claim side (the source side is
/// the [`SourceId`](forge_core_contracts::SourceId)).
///
/// Coarse by design (artifact path + cited id); the source id is the join key
/// and the artifact path is the audit address. See the module doc for why a
/// finer locator is a non-goal for the MVP.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimRef {
    /// The repo-relative path of the artifact that carries the `source_id`
    /// (e.g. `contracts/policies/memory-trust.yaml`).
    pub document_path: String,
    /// The cited source id, as it appears in the artifact. Echoed here so a
    /// single `ClaimRef` is self-describing without joining back to the graph
    /// key.
    pub source_id: String,
}

impl ClaimRef {
    /// Construct a [`ClaimRef`] from its two stable fields.
    #[must_use]
    pub fn new(document_path: impl Into<String>, source_id: impl Into<String>) -> Self {
        Self {
            document_path: document_path.into(),
            source_id: source_id.into(),
        }
    }
}

/// Build the Evidence Graph: `SourceId.0 → ClaimRefs` for every `source_id`
/// occurrence found in `documents`. The graph is **raw** — it indexes every
/// citation regardless of whether the id resolves. Resolution (fail-closed
/// rejection of unresolvable ids) is the job of the Citation Check
/// (`forge_core_validate::validate_yaml_citation_references`); this projection
/// is the "who cites whom" index that feeds `research graph` and impact
/// analysis. Callers that want only resolvable citations filter by membership
/// in `projection.sources` (runtime) ∪ the curated registry (passed separately
/// at the validate layer).
///
/// `projection` is accepted (not just documents) because the graph's contract
/// is "claims that cite a *registered* source" in the common case — but the
/// function does not drop unregistered citations, since a citation to an
/// unregistered id is itself the signal the Citation Check reports. The
/// projection is therefore available to callers that want to mark live vs
/// retired sources; this pure walk does not need it to build the raw index.
/// It is taken by reference to keep the signature symmetric with the citation
/// check and to leave room for a future `resolvable_only` filter without an
/// ABI change.
///
/// Deterministic: output is a `BTreeMap`, values sorted — same artifacts ⇒
/// byte-identical graph.
#[must_use]
pub fn evidence_graph(
    documents: &[ParsedYamlDocument],
    _projection: &ResearchProjection,
) -> BTreeMap<String, Vec<ClaimRef>> {
    let mut graph: BTreeMap<String, Vec<ClaimRef>> = BTreeMap::new();
    for document in documents {
        collect_claim_refs(&mut graph, &document.path, &document.value);
    }
    // Sort each bucket so the whole graph is deterministically ordered.
    for bucket in graph.values_mut() {
        bucket.sort();
    }
    graph
}

/// Walk a YAML value and, for every `source_id` string key, record a
/// [`ClaimRef`] under that id. Non-string `source_id` values are skipped here
/// (the Citation Check already reports them as `SourceIdMustBeString`); the
/// graph is an index of valid citations, not a second validator.
fn collect_claim_refs(
    graph: &mut BTreeMap<String, Vec<ClaimRef>>,
    document_path: &str,
    value: &Value,
) {
    match value {
        Value::Mapping(mapping) => {
            for (key, child) in mapping {
                if key.as_str() == Some("source_id") {
                    if let Some(source_id) = child.as_str() {
                        graph
                            .entry(source_id.to_owned())
                            .or_default()
                            .push(ClaimRef::new(document_path, source_id));
                    }
                }
                collect_claim_refs(graph, document_path, child);
            }
        }
        Value::Sequence(items) => {
            for item in items {
                collect_claim_refs(graph, document_path, item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::{ResearchSource, ResearchSourceKind, SourceId};

    fn empty_projection() -> ResearchProjection {
        ResearchProjection::default()
    }

    fn doc(path: &str, body: &str) -> ParsedYamlDocument {
        ParsedYamlDocument {
            path: path.to_string(),
            value: yaml_serde::from_str(body).expect("synthetic yaml"),
        }
    }

    fn projection_with(id: &str) -> ResearchProjection {
        // A projection carrying one live source, to confirm the projection arg
        // is accepted and does not change the raw-index behaviour.
        let source = ResearchSource {
            id: SourceId(id.into()),
            kind: ResearchSourceKind::Paper,
            title: "t".into(),
            locator: "https://example.org/x".into(),
            fetched_at: 1,
            content_hash: Some("sha256:0".into()),
            harvested_by: "agent.1".into(),
            trace_ref: Some("run.1".into()),
        };
        let mut p = ResearchProjection::default();
        p.sources.insert(id.into(), source);
        p
    }

    #[test]
    fn empty_documents_yield_empty_graph() {
        let graph = evidence_graph(&[], &empty_projection());
        assert!(graph.is_empty());
    }

    #[test]
    fn groups_citations_by_source_id() {
        let documents = [doc(
            "contracts/policies/a.yaml",
            r#"
schema_version: "0.1"
evidence_basis:
  direct_patterns:
    - source_id: "s.one"
    - source_id: "s.two"
    - source_id: "s.one"
"#,
        )];

        let graph = evidence_graph(&documents, &empty_projection());

        assert_eq!(graph.len(), 2);
        assert_eq!(graph["s.one"].len(), 2, "s.one cited twice");
        assert_eq!(graph["s.two"].len(), 1);
        // Echoed source_id matches the key.
        assert_eq!(graph["s.one"][0].source_id, "s.one");
        assert_eq!(graph["s.one"][0].document_path, "contracts/policies/a.yaml");
    }

    #[test]
    fn merges_citations_across_documents_sorted() {
        let documents = [
            doc(
                "contracts/policies/b.yaml",
                "evidence_basis:\n  direct_patterns:\n    - source_id: \"s.x\"\n",
            ),
            doc(
                "contracts/policies/a.yaml",
                "evidence_basis:\n  direct_patterns:\n    - source_id: \"s.x\"\n",
            ),
        ];

        let graph = evidence_graph(&documents, &empty_projection());
        let bucket = &graph["s.x"];
        assert_eq!(bucket.len(), 2);
        // Sorted by document_path: a.yaml before b.yaml.
        assert_eq!(bucket[0].document_path, "contracts/policies/a.yaml");
        assert_eq!(bucket[1].document_path, "contracts/policies/b.yaml");
    }

    #[test]
    fn raw_index_keeps_unregistered_citations() {
        // A citation to an id that is NOT in the projection is still indexed —
        // resolution is the Citation Check's job, not the graph's. The graph is
        // the "who cites whom" index, including dangling citations.
        let documents = [doc(
            "contracts/policies/c.yaml",
            "evidence_basis:\n  direct_patterns:\n    - source_id: \"ghost.neither\"\n",
        )];

        let graph = evidence_graph(&documents, &projection_with("s.live"));
        assert!(graph.contains_key("ghost.neither"));
        assert!(
            !graph.contains_key("s.live"),
            "uncited registered source has no bucket"
        );
    }

    #[test]
    fn non_string_source_id_is_skipped_not_indexed() {
        let documents = [doc(
            "contracts/policies/d.yaml",
            "evidence_basis:\n  direct_patterns:\n    - source_id: 42\n",
        )];

        let graph = evidence_graph(&documents, &empty_projection());
        assert!(
            graph.is_empty(),
            "non-string source_id must not produce a bucket"
        );
    }

    #[test]
    fn determinism_same_artifacts_same_graph() {
        let documents = [doc(
            "contracts/policies/e.yaml",
            "evidence_basis:\n  direct_patterns:\n    - source_id: \"s.one\"\n    - source_id: \"s.two\"\n",
        )];

        let first = evidence_graph(&documents, &empty_projection());
        let second = evidence_graph(&documents, &empty_projection());
        assert_eq!(first, second);
    }

    #[test]
    fn claim_ref_new_builds_fields() {
        let reference = ClaimRef::new("contracts/policies/x.yaml", "s.y");
        assert_eq!(reference.document_path, "contracts/policies/x.yaml");
        assert_eq!(reference.source_id, "s.y");
    }
}
