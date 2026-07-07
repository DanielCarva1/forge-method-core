//! Coordination-eval scorer over the 9 multi-agent dimensions (closes C5).
//!
//! Mirrors [`crate::eval`] (router scorer): the engine ships NO model (DC9), so
//! it cannot self-prove a coordination dimension. Instead it provides (a) a
//! structural validator ([`validate_coordination_contract`]), (b) a dangling-ref
//! coverage check ([`coordination_fixture_gaps`]), and (c) a scorer
//! ([`score_coordination`]) that applies the contract's
//! [`CoordinationEvalPassPolicy`] to HOST-PROVIDED per-dimension outcomes. The
//! host (an LLM, an operator, or a future MCP surface) supplies the outcome
//! evidence; the engine is the deterministic gate (DC9 / DD40).
//!
//! Outcomes are file-backed (DC10 / DD41): a different LLM can write an
//! outcomes payload, the engine scores it. The gate is the value.

use forge_core_contracts::common::RepoPath;
#[allow(unused_imports)] // PassPolicy used only in cfg(test)
use forge_core_contracts::coordination_eval::{
    CoordinationDimension, CoordinationEvalContract, CoordinationEvalPassPolicy,
    CoordinationMetricKind, CoordinationRequiredLevel,
};
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single host-provided outcome for one dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinationOutcome {
    pub dimension: CoordinationDimension,
    /// Did the evidence demonstrate the dimension holds?
    pub passed: bool,
    /// File-backed evidence (DC10) the host gathered proving the outcome.
    pub evidence: Vec<RepoPath>,
    /// Free-text rationale / notes (optional).
    pub notes: Option<String>,
}

/// The aggregate verdict after applying [`CoordinationEvalPassPolicy`].
///
/// Taxonomy (DD42): `Passed` requires every *required* dimension to pass.
/// `Failed` means at least one `must_pass` dimension failed. A `should_pass`
/// failure downgrades to a warning (never fails the release). A
/// `manual_review_required` open item blocks release only when
/// [`CoordinationEvalPassPolicy::manual_review_blocks_release`] is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationVerdict {
    Passed,
    Failed,
    ManualReviewRequired,
}

/// Aggregate score for a coordination eval run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinationScore {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub manual_review_open: usize,
    pub verdict: CoordinationVerdict,
    /// Warnings (e.g. a `should_pass` dim that failed — non-blocking).
    pub warnings: Vec<String>,
    /// Per-dimension outcomes, in contract order.
    pub outcomes: Vec<CoordinationOutcome>,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// A structural validation error on a coordination contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CoordinationValidationError {
    /// Fewer than all 9 dimensions present.
    MissingDimension { dimension: CoordinationDimension },
    /// A dimension appears more than once.
    DuplicateDimension { dimension: CoordinationDimension },
    /// `metric_kind == fixture_pass` but `fixture_refs` is empty.
    FixturePassMissingRefs { dimension: CoordinationDimension },
    /// `metric_kind == threshold` but `threshold` is `None`.
    ThresholdMissingValue { dimension: CoordinationDimension },
    /// `metric_kind == fixture_pass` but a `threshold` was set (inconsistent).
    FixturePassHasThreshold { dimension: CoordinationDimension },
    /// No `must_pass` dimension exists — the contract gates nothing.
    NoMustPassDimension,
}

/// Structurally validate a coordination contract before it is used as a gate.
///
/// Performs four independent checks and accumulates ALL problems (does not
/// short-circuit), so a single call surfaces every issue at once:
///
/// 1. **Duplicate dimensions** — each of the 9 canonical dimensions may appear
///    at most once.
/// 2. **Missing dimensions** — all 9 dimensions from [`CoordinationDimension::ALL`]
///    must be present.
/// 3. **`metric_kind` ↔ field consistency** — a `FixturePass` metric must have
///    non-empty `fixture_refs` and must NOT carry a `threshold`; a `Threshold`
///    or `LatencyBudget` metric must have a numeric `threshold`; `ManualReview`
///    has no field requirement.
/// 4. **At least one `must_pass` dimension** — a contract with no `must_pass`
///    dimension gates nothing and is rejected (`NoMustPassDimension`).
///
/// # Inputs
///
/// - `contract`: the [`CoordinationEvalContract`] to inspect (the 9-dimension
///   coordination suite authored by the host).
///
/// # Returns
///
/// A `Vec<CoordinationValidationError>`. An **empty** vector means the contract
/// is structurally sound and may be used as a gate. A non-empty vector means
/// the contract is broken — [`score_coordination`] will fail-closed on it (M1).
///
/// # Governance invariant
///
/// This is the first line of defense: a structurally invalid contract MUST NOT
/// be scored, because the verdict-chain in [`score_coordination`] would fall
/// through every guard and return a false `Passed` on an empty or broken
/// contract. `score_coordination` therefore calls this function at its top
/// (review S4.7 fix M1) and short-circuits to `Failed` on any error.
///
/// # Validation alignment
///
/// This function already **accumulates** (it collects every problem into the
/// returned `Vec` rather than bailing on the first), satisfying the repo's
/// "validation accumulates diagnostics" rule. It is intentionally NOT migrated
/// onto the canonical `forge_core_validate::ValidationReport`: it lives in a
/// different layer (`forge-core-decisions`), returns its own
/// [`CoordinationValidationError`] vocabulary that maps 1:1 onto scorer guard
/// failures, and serves as a structural gate for [`score_coordination`].
/// Flattening it into generic `Diagnostic`s would lose that mapping for no
/// benefit. Left on its own domain error type on purpose.
#[must_use]
pub fn validate_coordination_contract(
    contract: &CoordinationEvalContract,
) -> Vec<CoordinationValidationError> {
    let mut errs = Vec::new();

    // 1. duplicate detection (do this first so presence logic is sound).
    let mut seen = Vec::with_capacity(contract.dimensions.len());
    for dim in &contract.dimensions {
        if seen.contains(&dim.dimension) {
            errs.push(CoordinationValidationError::DuplicateDimension {
                dimension: dim.dimension,
            });
        } else {
            seen.push(dim.dimension);
        }
    }
    // 2. presence of all 9 canonical dims.
    for required in CoordinationDimension::ALL {
        if !seen.contains(&required) {
            errs.push(CoordinationValidationError::MissingDimension {
                dimension: required,
            });
        }
    }
    // 3. metric_kind <-> fields consistency.
    for dim in &contract.dimensions {
        match dim.metric_kind {
            CoordinationMetricKind::FixturePass => {
                if dim.fixture_refs.is_empty() {
                    errs.push(CoordinationValidationError::FixturePassMissingRefs {
                        dimension: dim.dimension,
                    });
                }
                if dim.threshold.is_some() {
                    errs.push(CoordinationValidationError::FixturePassHasThreshold {
                        dimension: dim.dimension,
                    });
                }
            }
            CoordinationMetricKind::Threshold => {
                if dim.threshold.is_none() {
                    errs.push(CoordinationValidationError::ThresholdMissingValue {
                        dimension: dim.dimension,
                    });
                }
            }
            CoordinationMetricKind::LatencyBudget => {
                // latency needs a numeric budget; we model it as a threshold.
                if dim.threshold.is_none() {
                    errs.push(CoordinationValidationError::ThresholdMissingValue {
                        dimension: dim.dimension,
                    });
                }
            }
            CoordinationMetricKind::ManualReview => {
                // no field requirement; manual review is open until a human signs.
            }
        }
    }
    // 4. the contract must gate SOMETHING (no must_pass == gates nothing).
    let has_must_pass = contract
        .dimensions
        .iter()
        .any(|d| d.required_level == CoordinationRequiredLevel::MustPass);
    if !has_must_pass {
        errs.push(CoordinationValidationError::NoMustPassDimension);
    }
    errs
}

/// Check that every `fixture_ref` and `evidence_ref` in the contract resolves
/// to a real file under `repo_root`. Returns the list of refs that do NOT
/// resolve (dangling or invalid). An empty return means the suite is REAL.
///
/// Mirrors [`crate::eval::corpus_coverage_gaps`] — the "lift from draft to real"
/// proof that no coordination dimension references a phantom file.
///
/// # Inputs
///
/// - `contract`: the coordination suite whose refs to check.
/// - `repo_root`: the repository root path against which repo-relative refs are
///   joined and tested for existence.
///
/// # Returns
///
/// `Vec<String>` of human-readable gap descriptions. Each string identifies the
/// ref, its kind (fixture/evidence), the dimension, and the reason (missing
/// file, absolute path, or backslash separator).
///
/// # Governance invariants
///
/// - **M4 (review S4.7)**: an ABSOLUTE ref (`/etc/passwd`, `C:\...`) makes
///   [`Path::join`] silently discard `repo_root` and test the host path instead,
///   producing a false REAL signal. Absolute refs, leading-backslash refs, and
///   any ref containing a backslash are flagged as gaps. Ref strings MUST be
///   repo-relative with forward slashes.
/// - **N1 (review S4.7 v2)**: the Windows drive-letter check requires `byte[1]`
///   == `:` AND `byte[2]` in `{/, \}` — a legitimate 2-char relative ref like
///   `a:b` is NOT a false gap.
#[must_use]
pub fn coordination_fixture_gaps(
    contract: &CoordinationEvalContract,
    repo_root: &Path,
) -> Vec<String> {
    let mut missing = Vec::new();
    let mut visit = |ref_str: &str, kind: &str, dim: CoordinationDimension| {
        // M4 (review S4.7): an ABSOLUTE ref (Unix leading '/' or a Windows
        // drive letter `C:\...`) makes Path::join DISCARD repo_root and test
        // the host path instead — a silent false-REAL signal. Reject absolute
        // refs as gaps; ref strings MUST be repo-relative. (Backslash ref
        // strings authored on Windows are also flagged: under a Unix repo_root
        // they are a single literal component and won't resolve.)
        // M4 (review S4.7): an ABSOLUTE ref makes Path::join DISCARD repo_root
        // and test the host path instead — a silent false-REAL signal. Detect
        // absolute-ness via STRING inspection (cross-platform): a leading `/`,
        // a leading `\`, a Windows drive-letter `C:\...`, OR the std
        // is_absolute() check. Reject absolute refs as gaps; ref strings MUST
        // be repo-relative. (A backslash ref authored on Windows won't resolve
        // under a Unix repo_root either.)
        let is_absolute_like = ref_str.starts_with('/')
            || ref_str.starts_with('\\')
            // N1 (review S4.7 v2): require a real Windows drive path — byte[1]
            // == ':' AND byte[2] is a separator — so a legitimate 2-char
            // relative ref like `a:b` is NOT a false gap.
            || (ref_str.as_bytes().get(1).is_some_and(|&c| c == b':')
                && ref_str.as_bytes().get(2).is_some_and(|&c| c == b'/' || c == b'\\'))
            || Path::new(ref_str).is_absolute();
        if is_absolute_like || ref_str.contains('\\') {
            missing.push(format!(
                "{kind} ref '{ref_str}' for dimension {dim:?} is absolute or contains a backslash — refs MUST be repo-relative with forward slashes",
            ));
            return;
        }
        let p = repo_root.join(ref_str);
        if !p.exists() {
            missing.push(format!(
                "{kind} ref '{ref_str}' for dimension {dim:?} does not resolve under {repo_root}",
                repo_root = repo_root.display()
            ));
        }
    };
    for dim in &contract.dimensions {
        for f in &dim.fixture_refs {
            visit(&f.0, "fixture", dim.dimension);
        }
        for e in &dim.evidence_refs {
            visit(&e.0, "evidence", dim.dimension);
        }
    }
    missing
}

/// Score a coordination contract against HOST-PROVIDED per-dimension outcomes,
/// producing a typed [`CoordinationVerdict`].
///
/// The engine ships no model (DC9 / DD40): it cannot self-prove a coordination
/// dimension. Instead, the host (an LLM, operator, or future MCP surface)
/// supplies evidence via `outcome_fn`; the engine applies the contract's
/// [`CoordinationEvalPassPolicy`] deterministically.
///
/// # Inputs
///
/// - `contract`: the [`CoordinationEvalContract`] defining the 9 dimensions,
///   their required levels, and the pass policy.
/// - `outcome_fn`: a closure mapping each [`CoordinationDimension`] to an
///   optional [`CoordinationOutcome`]. Returning `None` means "no evidence
///   supplied yet" for that dimension.
///
/// # Returns
///
/// A [`CoordinationScore`] carrying the verdict, per-dimension outcomes,
/// warnings, and a human-readable summary.
///
/// # Verdict logic (DD42)
///
/// - **Passed**: no `must_pass` dimension failed, no blocking manual-review
///   item is open, and (if `all_must_pass_dimensions_required`) no
///   non-required dimension is missing evidence.
/// - **Failed**: at least one `must_pass` dimension failed (including a MISSING
///   outcome — fail-closed).
/// - **`ManualReviewRequired`**: no hard failure, but an open manual-review item
///   exists AND `manual_review_blocks_release` is true.
///
/// # Governance invariants
///
/// - **M1 (review S4.7)**: validates the contract structurally at the top. An
///   empty or structurally broken contract short-circuits to `Failed` with the
///   structural errors as warnings — the gate MUST be fail-closed.
/// - **L5 (review S4.7)**: synthesized entries for missing outcomes use
///   `passed = false` so the audit trail never claims unevidenced success.
///   `should_pass` counting only increments `passed` when EVIDENCED.
/// - **L2 (review S4.7)**: a `debug_assert!` catches host-closure wiring
///   mistakes where `outcome_fn` returns an outcome for the wrong dimension.
/// - A `should_pass` failure is a WARNING, never a hard fail (DD42).
#[must_use]
pub fn score_coordination<F>(
    contract: &CoordinationEvalContract,
    outcome_fn: F,
) -> CoordinationScore
where
    F: Fn(&CoordinationDimension) -> Option<CoordinationOutcome>,
{
    // M1 (review S4.7): a governance gate MUST be fail-closed. Score on an
    // UNVALIDATED contract would return Passed on an empty/structurally-broken
    // contract (verdict chain falls through every guard). Validate first; on
    // any structural error short-circuit to a Failed verdict carrying the
    // problems as warnings so the host sees WHY the gate rejected.
    let structural = validate_coordination_contract(contract);
    if !structural.is_empty() {
        let warnings: Vec<String> = structural
            .iter()
            .map(|e| format!("structural contract error: {e:?}"))
            .collect();
        return CoordinationScore {
            total: contract.dimensions.len(),
            passed: 0,
            failed: contract.dimensions.len(),
            manual_review_open: 0,
            verdict: CoordinationVerdict::Failed,
            warnings,
            outcomes: Vec::new(),
            summary: format!(
                "failed: contract failed structural validation ({} errors)",
                structural.len()
            ),
        };
    }

    let policy = &contract.pass_policy;
    let mut outcomes = Vec::with_capacity(contract.dimensions.len());
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut manual_review_open = 0usize;
    // count of non-required dims with NO supplied outcome (relevant only when
    // all_must_pass_dimensions_required is true — evidence incomplete).
    let mut missing_non_required = 0usize;
    let mut warnings = Vec::new();
    let mut hard_failed = false;

    for dim in &contract.dimensions {
        let outcome = outcome_fn(&dim.dimension);
        let level = dim.required_level;

        // Resolve an effective passed flag. A missing outcome is fail-closed
        // for must_pass; for should_pass it is a warning; for manual review it
        // is an OPEN item.
        let (eff_passed, has_outcome) = match (&outcome, level) {
            (Some(o), _) => (o.passed, true),
            (
                None,
                CoordinationRequiredLevel::MustPass
                | CoordinationRequiredLevel::ManualReviewRequired,
            ) => (false, false),
            (None, CoordinationRequiredLevel::ShouldPass) => (true, false),
        };

        match level {
            CoordinationRequiredLevel::MustPass => {
                if eff_passed {
                    passed += 1;
                } else {
                    failed += 1;
                    hard_failed = true;
                    if !has_outcome {
                        warnings.push(format!(
                            "{:?}: no outcome supplied (must_pass treated as failed)",
                            dim.dimension
                        ));
                    }
                }
            }
            CoordinationRequiredLevel::ShouldPass => {
                if !has_outcome {
                    missing_non_required += 1;
                }
                // L5 (review S4.7): only count as passed when EVIDENCED. A
                // missing should_pass outcome is unevidenced (synthesized
                // entry reads passed=false); do not inflate the passed count.
                if has_outcome && eff_passed {
                    passed += 1;
                } else if has_outcome {
                    // evidenced should_pass FAILURE is a WARNING, never a hard fail (DD42).
                    warnings.push(format!(
                        "{:?}: should_pass dimension did not pass (non-blocking)",
                        dim.dimension
                    ));
                }
                // !has_outcome: unevidenced — tracked via missing_non_required;
                // silent here (escalates only when all_must_pass_required).
            }
            CoordinationRequiredLevel::ManualReviewRequired => {
                if eff_passed {
                    passed += 1;
                } else {
                    manual_review_open += 1;
                    if !has_outcome {
                        warnings.push(format!(
                            "{:?}: manual review pending (no outcome supplied)",
                            dim.dimension
                        ));
                    }
                }
            }
        }
        if let Some(o) = outcome.clone() {
            // L2 (review S4.7): catch host-closure wiring mistakes cheaply. A
            // returned outcome for a DIFFERENT dimension would silently apply
            // its verdict to the wrong dim.
            debug_assert!(
                o.dimension == dim.dimension,
                "outcome_fn returned outcome for {:?} while querying {:?}",
                o.dimension,
                dim.dimension
            );
            outcomes.push(o);
        } else {
            // L5 (review S4.7): a synthesized "no evidence" entry must read
            // passed=false in the audit trail (never claim success unevidenced).
            // The counters below already encode the real (non-blocking /
            // fail-closed) semantics; the entry just needs to be honest.
            outcomes.push(CoordinationOutcome {
                dimension: dim.dimension,
                passed: false,
                evidence: Vec::new(),
                notes: Some("no host outcome supplied".to_string()),
            });
        }
    }

    let total = contract.dimensions.len();
    let verdict = if hard_failed {
        CoordinationVerdict::Failed
    } else if manual_review_open > 0 && policy.manual_review_blocks_release {
        CoordinationVerdict::ManualReviewRequired
    } else if policy.all_must_pass_dimensions_required && missing_non_required > 0 {
        // No must_pass failed, but some non-required dim has NO supplied
        // outcome and the contract demands complete evidence — treat as
        // manual-review-pending (evidence incomplete), not Passed.
        CoordinationVerdict::ManualReviewRequired
    } else {
        CoordinationVerdict::Passed
    };

    let summary = format!(
        "{}: {passed}/{total} passed, {failed} failed, {manual_review_open} manual-review open",
        verdict_snake(verdict)
    );

    CoordinationScore {
        total,
        passed,
        failed,
        manual_review_open,
        verdict,
        warnings,
        outcomes,
        summary,
    }
}

/// Lowercase `snake_case` name for a [`CoordinationVerdict`], mirroring its
/// serde `rename_all = "snake_case"` so the human summary matches the
/// serialized JSON form (review S4.7 L3).
fn verdict_snake(v: CoordinationVerdict) -> &'static str {
    match v {
        CoordinationVerdict::Passed => "passed",
        CoordinationVerdict::Failed => "failed",
        CoordinationVerdict::ManualReviewRequired => "manual_review_required",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::common::StableId;

    fn sample_contract() -> CoordinationEvalContract {
        let dim = |d: CoordinationDimension, level: CoordinationRequiredLevel| {
            forge_core_contracts::coordination_eval::CoordinationEvalDimension {
                dimension: d,
                metric_kind: CoordinationMetricKind::FixturePass,
                required_level: level,
                fixture_refs: vec![RepoPath(
                    format!("fixtures/{d:?}.yaml").to_ascii_lowercase(),
                )],
                threshold: None,
                failure_signal: format!("{d:?} failed"),
                evidence_refs: vec![],
            }
        };
        CoordinationEvalContract {
            id: StableId("eval.test".into()),
            contract_ref: RepoPath("contracts/evals/x.yaml".into()),
            status: forge_core_contracts::coordination_eval::CoordinationEvalStatus::Required,
            dimensions: CoordinationDimension::ALL
                .map(|d| dim(d, CoordinationRequiredLevel::MustPass))
                .to_vec(),
            pass_policy: CoordinationEvalPassPolicy {
                required_level: CoordinationRequiredLevel::MustPass,
                all_must_pass_dimensions_required: true,
                manual_review_blocks_release: true,
            },
        }
    }

    #[test]
    fn valid_contract_has_no_errors() {
        let errs = validate_coordination_contract(&sample_contract());
        assert!(errs.is_empty(), "expected no errors, got {errs:?}");
    }

    #[test]
    fn missing_dimension_reported() {
        let mut c = sample_contract();
        c.dimensions.remove(0); // drop one dim
        let errs = validate_coordination_contract(&c);
        assert!(
            errs.iter()
                .any(|e| matches!(e, CoordinationValidationError::MissingDimension { .. })),
            "expected MissingDimension, got {errs:?}"
        );
    }

    #[test]
    fn duplicate_dimension_reported() {
        let mut c = sample_contract();
        c.dimensions.push(c.dimensions[0].clone());
        let errs = validate_coordination_contract(&c);
        assert!(errs
            .iter()
            .any(|e| matches!(e, CoordinationValidationError::DuplicateDimension { .. })));
    }

    #[test]
    fn fixture_pass_with_empty_refs_reported() {
        let mut c = sample_contract();
        c.dimensions[0].fixture_refs.clear();
        let errs = validate_coordination_contract(&c);
        assert!(errs.iter().any(|e| matches!(
            e,
            CoordinationValidationError::FixturePassMissingRefs { .. }
        )));
    }

    #[test]
    fn threshold_metric_without_value_reported() {
        let mut c = sample_contract();
        c.dimensions[0].metric_kind = CoordinationMetricKind::Threshold;
        c.dimensions[0].fixture_refs.clear();
        let errs = validate_coordination_contract(&c);
        assert!(errs
            .iter()
            .any(|e| matches!(e, CoordinationValidationError::ThresholdMissingValue { .. })));
    }

    #[test]
    fn no_must_pass_reported() {
        let mut c = sample_contract();
        for d in &mut c.dimensions {
            d.required_level = CoordinationRequiredLevel::ShouldPass;
        }
        let errs = validate_coordination_contract(&c);
        assert!(errs
            .iter()
            .any(|e| matches!(e, CoordinationValidationError::NoMustPassDimension)));
    }

    #[test]
    fn score_all_pass_yields_passed() {
        let c = sample_contract();
        let s = score_coordination(&c, |dim| {
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: true,
                evidence: vec![],
                notes: None,
            })
        });
        assert_eq!(s.verdict, CoordinationVerdict::Passed);
        assert_eq!(s.passed, 9);
        assert_eq!(s.failed, 0);
        assert_eq!(s.manual_review_open, 0);
    }

    #[test]
    fn score_one_must_pass_fail_yields_failed() {
        let c = sample_contract();
        let s = score_coordination(&c, |dim| {
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: *dim != CoordinationDimension::LaneCollisionDetection,
                evidence: vec![],
                notes: None,
            })
        });
        assert_eq!(s.verdict, CoordinationVerdict::Failed);
        assert_eq!(s.failed, 1);
    }

    #[test]
    fn should_pass_failure_is_warning_not_fail() {
        let mut c = sample_contract();
        // make one dim should_pass
        c.dimensions[0].required_level = CoordinationRequiredLevel::ShouldPass;
        let s = score_coordination(&c, |dim| {
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: *dim != c.dimensions[0].dimension,
                evidence: vec![],
                notes: None,
            })
        });
        // the should_pass dim failed -> warning, but the other 8 must_pass passed
        assert_eq!(s.verdict, CoordinationVerdict::Passed);
        assert!(s.warnings.iter().any(|w| w.contains("should_pass")));
    }

    #[test]
    fn missing_must_pass_outcome_is_fail_closed() {
        let c = sample_contract();
        // supply NO outcomes at all
        let s = score_coordination(&c, |_| None);
        assert_eq!(s.verdict, CoordinationVerdict::Failed);
        assert_eq!(s.failed, 9);
    }

    #[test]
    fn manual_review_open_blocks_release() {
        let mut c = sample_contract();
        c.dimensions[0].required_level = CoordinationRequiredLevel::ManualReviewRequired;
        // all other dims pass, the manual-review one is OPEN (no outcome)
        let s = score_coordination(&c, |dim| {
            if *dim == c.dimensions[0].dimension {
                return None; // open
            }
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: true,
                evidence: vec![],
                notes: None,
            })
        });
        assert_eq!(s.verdict, CoordinationVerdict::ManualReviewRequired);
        assert_eq!(s.manual_review_open, 1);
    }

    #[test]
    fn manual_review_open_does_not_block_when_policy_allows() {
        let mut c = sample_contract();
        c.pass_policy.manual_review_blocks_release = false;
        c.dimensions[0].required_level = CoordinationRequiredLevel::ManualReviewRequired;
        let s = score_coordination(&c, |dim| {
            if *dim == c.dimensions[0].dimension {
                return None;
            }
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: true,
                evidence: vec![],
                notes: None,
            })
        });
        // open manual-review but policy says don't block -> Passed (with note)
        assert_eq!(s.verdict, CoordinationVerdict::Passed);
    }

    // --- real suite integration (C5 closure) -------------------------------

    fn real_suite() -> CoordinationEvalContract {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/evals/minimal-coordination-eval-suite.yaml")
            .canonicalize()
            .expect("suite must exist");
        let text = std::fs::read_to_string(&path).expect("read suite");
        let doc: forge_core_contracts::coordination_eval::CoordinationEvalContractDocument =
            yaml_serde::from_str(&text).expect("deserialize suite");
        doc.coordination_eval_contract
    }

    #[test]
    fn real_suite_loads_and_is_structurally_valid() {
        let c = real_suite();
        let errs = validate_coordination_contract(&c);
        assert!(
            errs.is_empty(),
            "real suite has structural errors: {errs:?}"
        );
        assert_eq!(c.dimensions.len(), 9, "all 9 dims present");
    }

    #[test]
    fn real_suite_has_no_dangling_refs() {
        // This is the "lift from draft to real" proof: every fixture/evidence
        // ref resolves to a real file. C5 success signal.
        let c = real_suite();
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root");
        let gaps = coordination_fixture_gaps(&c, &repo_root);
        assert!(gaps.is_empty(), "real suite has dangling refs: {gaps:?}");
    }

    #[test]
    fn real_suite_yields_passed_with_full_outcomes() {
        // C5 formal closure: the eval machinery produces a typed Passed verdict
        // over the real suite when the host supplies passing outcomes.
        let c = real_suite();
        let s = score_coordination(&c, |dim| {
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: true,
                evidence: vec![],
                notes: Some("host-provided passing evidence".into()),
            })
        });
        assert_eq!(
            s.verdict,
            CoordinationVerdict::Passed,
            "summary: {}",
            s.summary
        );
        assert_eq!(s.total, 9);
        assert_eq!(s.failed, 0);
    }

    // --- review S4.7 fixes ------------------------------------------------

    #[test]
    fn score_empty_contract_is_failed_not_passed() {
        // M1: a governance gate MUST NOT return Passed on an empty contract.
        let empty = CoordinationEvalContract {
            id: StableId("eval.empty".into()),
            contract_ref: RepoPath("x.yaml".into()),
            status: forge_core_contracts::coordination_eval::CoordinationEvalStatus::Required,
            dimensions: Vec::new(),
            pass_policy: CoordinationEvalPassPolicy {
                required_level: CoordinationRequiredLevel::MustPass,
                all_must_pass_dimensions_required: true,
                manual_review_blocks_release: true,
            },
        };
        let s = score_coordination(&empty, |_| None);
        assert_eq!(
            s.verdict,
            CoordinationVerdict::Failed,
            "summary: {}",
            s.summary
        );
        assert!(!s.warnings.is_empty(), "must explain why it failed");
    }

    #[test]
    fn score_structurally_invalid_contract_is_failed() {
        // M1: duplicate dim + missing dims would pass validate; score must reject.
        let mut c = sample_contract();
        c.dimensions.push(c.dimensions[0].clone()); // duplicate
        let s = score_coordination(&c, |dim| {
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: true,
                evidence: vec![],
                notes: None,
            })
        });
        assert_eq!(s.verdict, CoordinationVerdict::Failed);
    }

    #[test]
    fn absolute_ref_is_reported_as_gap() {
        // M4: an absolute ref must NOT silently resolve against the host path
        // and report a false REAL. It must be flagged as a gap.
        let mut c = sample_contract();
        c.dimensions[0].fixture_refs = vec![RepoPath("/etc/passwd".into())];
        let tmp = std::env::temp_dir();
        let gaps = coordination_fixture_gaps(&c, &tmp);
        assert!(
            gaps.iter()
                .any(|g| g.contains("absolute") || g.contains("backslash")),
            "absolute ref must be flagged, got: {gaps:?}"
        );
    }

    #[test]
    fn backslash_ref_is_reported_as_gap() {
        // M4 echo: a Windows-authored backslash ref won't resolve on Unix.
        let mut c = sample_contract();
        c.dimensions[0].fixture_refs = vec![RepoPath("docs\\fixtures\\x.yaml".into())];
        let tmp = std::env::temp_dir();
        let gaps = coordination_fixture_gaps(&c, &tmp);
        assert!(gaps.iter().any(|g| g.contains("backslash")));
    }

    #[test]
    fn summary_verdict_is_snake_case() {
        // L3: summary must use lowercase to match the serde form.
        let c = sample_contract();
        let s = score_coordination(&c, |dim| {
            Some(CoordinationOutcome {
                dimension: *dim,
                passed: true,
                evidence: vec![],
                notes: None,
            })
        });
        assert!(s.summary.starts_with("passed:"), "summary: {}", s.summary);
        assert!(!s.summary.contains("Passed"));
    }
}
