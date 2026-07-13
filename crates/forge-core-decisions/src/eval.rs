//! Router eval corpus + measurement harness (resolves grill R2).
//!
//! The engine ships NO model (DC9), so it cannot run the router itself. Instead
//! it provides: (a) a typed eval corpus ([`EvalCorpusDocument`]) of utterance ->
//! expected-workflow cases, and (b) a scorer ([`score_router`]) that measures
//! how often a host-provided router fn matches expected. The C1 accuracy bar
//! (>=90%) is REPORTED by running the target host against this corpus, not
//! asserted in CI (no LLM in core).

use crate::load_catalog;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single eval case: a free-text utterance and the workflow id a correct
/// router should recommend, tagged with the funnel phase it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalCase {
    pub utterance: String,
    pub expected_workflow: String,
    pub phase: String,
}

/// The corpus document on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalCorpusDocument {
    pub schema_version: String,
    pub eval_corpus: Vec<EvalCase>,
}

/// A per-case scorer outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseScore {
    pub utterance: String,
    pub expected: String,
    pub actual: Option<String>,
    pub correct: bool,
}

/// An aggregate router score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouterScore {
    pub total: usize,
    pub correct: usize,
    /// accuracy in basis points (0..=10000) to avoid floats in the core.
    pub accuracy_bps: u32,
    pub cases: Vec<CaseScore>,
}

impl RouterScore {
    /// Accuracy as a percentage (0.0..=100.0).
    #[must_use]
    pub fn accuracy_percent(&self) -> f64 {
        f64::from(self.accuracy_bps) / 100.0
    }
}

/// Hand-rolled error enum for [`load_eval_corpus`]. Replaces the legacy
/// `Result<_, String>` signature so callers get typed variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalCorpusLoadError {
    /// `fs::read_to_string` failed; carries the lossy io error string.
    Read { source: String },
    /// `yaml_serde::from_str` failed; carries the lossy deserialize error string.
    Deserialize { source: String },
}

impl std::fmt::Display for EvalCorpusLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { source } => write!(formatter, "read error: {source}"),
            Self::Deserialize { source } => {
                write!(formatter, "deserialize error: {source}")
            }
        }
    }
}

impl std::error::Error for EvalCorpusLoadError {}

/// Load an eval corpus from a YAML file.
///
/// # Errors
///
/// Returns [`EvalCorpusLoadError`] if the file cannot be read or deserialized.
pub fn load_eval_corpus(path: &Path) -> Result<EvalCorpusDocument, EvalCorpusLoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| EvalCorpusLoadError::Read {
        source: source.to_string(),
    })?;
    yaml_serde::from_str(&text).map_err(|source| EvalCorpusLoadError::Deserialize {
        source: source.to_string(),
    })
}

/// Score a router closure against the corpus. `router` maps an utterance to the
/// workflow id it recommends (None if it declines). This is how an operator
/// measures a host's accuracy against the C1 bar.
pub fn score_router<F>(corpus: &[EvalCase], router: F) -> RouterScore
where
    F: Fn(&str) -> Option<String>,
{
    let mut cases = Vec::with_capacity(corpus.len());
    let mut correct = 0usize;
    for c in corpus {
        let actual = router(&c.utterance);
        let is_correct = actual.as_deref() == Some(c.expected_workflow.as_str());
        if is_correct {
            correct += 1;
        }
        cases.push(CaseScore {
            utterance: c.utterance.clone(),
            expected: c.expected_workflow.clone(),
            actual,
            correct: is_correct,
        });
    }
    let total = corpus.len();
    let accuracy_bps = accuracy_bps(correct, total);
    RouterScore {
        total,
        correct,
        accuracy_bps,
        cases,
    }
}

fn accuracy_bps(correct: usize, total: usize) -> u32 {
    if total == 0 {
        return 0;
    }

    let scaled = ((correct as u128) * 10_000) / (total as u128);
    u32::try_from(scaled).unwrap_or(0)
}

/// Validate that every `expected_workflow` in the corpus resolves in the catalog.
/// Returns the list of unresolved ids (empty == valid).
#[must_use]
pub fn corpus_coverage_gaps(corpus: &[EvalCase], catalog_dir: &Path) -> Vec<String> {
    let report = load_catalog(catalog_dir);
    let known: std::collections::HashSet<&str> = report
        .catalog
        .entries
        .iter()
        .map(|e| e.id.0.as_str())
        .collect();
    corpus
        .iter()
        .filter(|c| !known.contains(c.expected_workflow.as_str()))
        .map(|c| c.expected_workflow.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/evidence/workflow-retirement/legacy-catalog")
            .canonicalize()
            .unwrap()
    }
    fn corpus_file() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/eval/router-eval-corpus.yaml")
            .canonicalize()
            .expect("eval corpus must exist at contracts/eval/router-eval-corpus.yaml")
    }

    #[test]
    fn corpus_loads_and_spans_all_six_phases() {
        let doc = load_eval_corpus(&corpus_file()).expect("load corpus");
        assert!(
            doc.eval_corpus.len() >= 40,
            "C1 corpus needs >=40 cases, got {}",
            doc.eval_corpus.len()
        );
        let phases: std::collections::HashSet<&str> =
            doc.eval_corpus.iter().map(|c| c.phase.as_str()).collect();
        for required in [
            "1-discovery",
            "2-specification",
            "3-plan",
            "4-build-verify",
            "5-ready-operate",
            "6-evolve",
        ] {
            assert!(phases.contains(required), "corpus missing phase {required}");
        }
    }

    #[test]
    fn every_expected_workflow_resolves_in_catalog() {
        let doc = load_eval_corpus(&corpus_file()).expect("load corpus");
        let gaps = corpus_coverage_gaps(&doc.eval_corpus, &corpus_dir());
        assert!(
            gaps.is_empty(),
            "corpus references unknown workflows: {gaps:?}"
        );
    }

    #[test]
    fn accuracy_bps_handles_large_corpora_without_overflow() {
        let large_total = usize::MAX;

        let even_large_total = large_total - 1;

        assert_eq!(accuracy_bps(large_total, large_total), 10_000);
        assert_eq!(accuracy_bps(even_large_total / 2, even_large_total), 5_000);
        assert_eq!(accuracy_bps(1, 0), 0);
    }

    #[test]
    fn scorer_smoke_with_stub_router() {
        // A stub router that always recommends the first case's expected.
        let corpus = [
            EvalCase {
                utterance: "plan a sprint".into(),
                expected_workflow: "plan-sprint".into(),
                phase: "3-plan".into(),
            },
            EvalCase {
                utterance: "discover intent".into(),
                expected_workflow: "discover-intent".into(),
                phase: "1-discovery".into(),
            },
        ];
        let first = corpus[0].expected_workflow.clone();
        let score = score_router(&corpus, |_| Some(first.clone()));
        assert_eq!(score.total, 2);
        assert_eq!(score.correct, 1);
        assert_eq!(score.accuracy_bps, 5000); // 50.00%
        assert!(
            (score.accuracy_percent() - 50.0).abs() < 1e-9,
            "accuracy percent drift"
        );
    }
}
