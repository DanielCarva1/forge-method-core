use forge_core_contracts::eval_run::{EvalFailureCluster, EvalVerdict};
use forge_core_contracts::{EvalRunContractDocument, RepoPath, StableId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

pub const EVAL_COMPARE_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvalArmLabel {
    SingleAgent,
    Graph,
    Mas,
    Manual,
}

impl EvalArmLabel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleAgent => "single-agent",
            Self::Graph => "graph",
            Self::Mas => "mas",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for EvalArmLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEvalArmLabelError {
    pub value: String,
}

impl fmt::Display for ParseEvalArmLabelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported eval arm '{}'; expected one of: single-agent, graph, mas, manual",
            self.value
        )
    }
}

impl std::error::Error for ParseEvalArmLabelError {}

impl FromStr for EvalArmLabel {
    type Err = ParseEvalArmLabelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "single-agent" | "single_agent" | "singleagent" => Ok(Self::SingleAgent),
            "graph" | "workflow-graph" | "workflow_graph" => Ok(Self::Graph),
            "mas" | "multi-agent" | "multi_agent" => Ok(Self::Mas),
            "manual" | "human" => Ok(Self::Manual),
            other => Err(ParseEvalArmLabelError {
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCompareSuiteDocument {
    pub schema_version: String,
    pub eval_compare_suite: EvalCompareSuite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCompareSuite {
    pub id: StableId,
    pub comparison_id: StableId,
    pub baseline: EvalArmSpec,
    pub candidate: EvalArmSpec,
    pub policy: EvalComparePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalArmSpec {
    pub label: EvalArmLabel,
    pub run_refs: Vec<RepoPath>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalComparePolicy {
    pub require_matching_tasks: bool,
    pub require_evidence_refs: bool,
    pub require_trace_refs: bool,
    pub minimum_task_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalCompareStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalRecommendation {
    KeepBaseline,
    TryCandidate,
    BlockCandidate,
    Inconclusive,
}

// ---------------------------------------------------------------------------
// Diagnostics — migrated to the canonical `forge_core_validate` types (V2.B).
//
// `EvalDiagnostic`, `EvalDiagnosticSeverity`, and `EvalDiagnosticCode` were
// near-identical clones of `Diagnostic` / `DiagnosticSeverity` / `DiagnosticCode`.
// They are now aliases for the canonical types: the eval-specific code variants
// live in `forge_core_validate::DiagnosticCode` (prefixed `Eval*`), and each
// carries an explicit `#[serde(rename)]` so the snake_case wire string (e.g.
// `missing_evidence_file`) is byte-identical to the deleted enum's output.
// ---------------------------------------------------------------------------

/// Canonical diagnostic, re-exported so eval callers keep their existing
/// import path (`EvalDiagnostic`) without touching call sites.
pub type EvalDiagnostic = forge_core_validate::Diagnostic;
/// Canonical diagnostic severity.
pub type EvalDiagnosticSeverity = forge_core_validate::DiagnosticSeverity;
/// Canonical diagnostic code (the eval-specific variants are `Eval*`-prefixed
/// members of the canonical enum, renamed to their original wire strings).
pub type EvalDiagnosticCode = forge_core_validate::DiagnosticCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalRunInput {
    pub source_ref: RepoPath,
    pub document: EvalRunContractDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvalComparisonReport {
    pub schema_version: String,
    pub comparison_id: String,
    pub suite_id: String,
    pub baseline: EvalArmLabel,
    pub candidate: EvalArmLabel,
    pub source: String,
    pub status: EvalCompareStatus,
    pub task_count: usize,
    pub baseline_summary: EvalArmSummary,
    pub candidate_summary: EvalArmSummary,
    pub deltas: EvalMetricDeltas,
    pub recommendation: EvalRecommendation,
    pub policy_reasons: Vec<String>,
    pub measurement_gaps: Vec<String>,
    pub input_refs: Vec<String>,
    pub diagnostics: Vec<EvalDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvalArmSummary {
    pub label: EvalArmLabel,
    pub run_count: usize,
    pub task_count: usize,
    pub successes: usize,
    pub failures: usize,
    pub partials: usize,
    pub errors: usize,
    pub success_rate_bps: u32,
    pub total_cost_usd_micros: u64,
    pub mean_cost_usd_micros: u64,
    pub total_wall_time_ms: u64,
    pub mean_wall_time_ms: u64,
    pub total_tool_calls: u64,
    pub mean_tool_calls_per_run_bps: u64,
    pub total_turns: u64,
    pub mean_turns_per_run_bps: u64,
    pub total_tokens: u64,
    pub mean_tokens_per_run: u64,
    pub failure_clusters: BTreeMap<String, usize>,
    pub evidence_refs: Vec<String>,
    pub trace_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvalMetricDeltas {
    pub success_rate_bps: i64,
    pub total_cost_usd_micros: i128,
    pub mean_cost_usd_micros: i128,
    pub total_wall_time_ms: i128,
    pub mean_wall_time_ms: i128,
    pub total_tool_calls: i128,
    pub total_turns: i128,
    pub total_tokens: i128,
}

/// Compares two sets of precomputed eval runs under the supplied suite policy.
#[must_use]
pub fn compare_eval_runs(
    suite: &EvalCompareSuite,
    requested_baseline: EvalArmLabel,
    requested_candidate: EvalArmLabel,
    baseline_runs: &[EvalRunInput],
    candidate_runs: &[EvalRunInput],
) -> EvalComparisonReport {
    compare_eval_runs_with_diagnostics(
        suite,
        requested_baseline,
        requested_candidate,
        baseline_runs,
        candidate_runs,
        Vec::new(),
    )
}

/// Compares precomputed eval runs while preserving caller-supplied boundary
/// diagnostics in the final fail-closed recommendation.
#[must_use]
pub fn compare_eval_runs_with_diagnostics(
    suite: &EvalCompareSuite,
    requested_baseline: EvalArmLabel,
    requested_candidate: EvalArmLabel,
    baseline_runs: &[EvalRunInput],
    candidate_runs: &[EvalRunInput],
    extra_diagnostics: Vec<EvalDiagnostic>,
) -> EvalComparisonReport {
    let mut diagnostics = Vec::new();
    if suite.baseline.label != requested_baseline {
        diagnostics.push(EvalDiagnostic::error(
            EvalDiagnosticCode::EvalBaselineLabelMismatch,
            "eval_compare_suite.baseline.label",
            format!(
                "suite baseline is {}, but command requested {}",
                suite.baseline.label, requested_baseline
            ),
        ));
    }
    if suite.candidate.label != requested_candidate {
        diagnostics.push(EvalDiagnostic::error(
            EvalDiagnosticCode::EvalCandidateLabelMismatch,
            "eval_compare_suite.candidate.label",
            format!(
                "suite candidate is {}, but command requested {}",
                suite.candidate.label, requested_candidate
            ),
        ));
    }

    validate_run_set(&mut diagnostics, "baseline", baseline_runs, &suite.policy);
    validate_run_set(&mut diagnostics, "candidate", candidate_runs, &suite.policy);
    if suite.policy.require_matching_tasks {
        validate_matching_tasks(&mut diagnostics, baseline_runs, candidate_runs);
    }
    diagnostics.extend(extra_diagnostics);

    let baseline_summary = summarize_runs(suite.baseline.label, baseline_runs);
    let candidate_summary = summarize_runs(suite.candidate.label, candidate_runs);
    let deltas = metric_deltas(&baseline_summary, &candidate_summary);
    let mut measurement_gaps = vec![
        "human_intervention_count is not represented in EvalRunContract v0.1".to_string(),
        "quality scoring is limited to recorded verdicts and optional quality_signals".to_string(),
    ];
    if baseline_runs.len() < 3 || candidate_runs.len() < 3 {
        measurement_gaps.push(
            "sample size is below three tasks; recommendation remains conservative".to_string(),
        );
    }

    let has_errors = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == EvalDiagnosticSeverity::Error);
    let (recommendation, policy_reasons) =
        recommend_candidate(has_errors, &baseline_summary, &candidate_summary, &deltas);
    let status = if has_errors {
        EvalCompareStatus::Blocked
    } else {
        EvalCompareStatus::Passed
    };
    let input_refs = baseline_runs
        .iter()
        .chain(candidate_runs.iter())
        .map(|input| input.source_ref.0.clone())
        .collect();

    EvalComparisonReport {
        schema_version: EVAL_COMPARE_SCHEMA_VERSION.to_string(),
        comparison_id: suite.comparison_id.0.clone(),
        suite_id: suite.id.0.clone(),
        baseline: requested_baseline,
        candidate: requested_candidate,
        source: "precomputed_eval_runs".to_string(),
        status,
        task_count: shared_task_count(baseline_runs, candidate_runs),
        baseline_summary,
        candidate_summary,
        deltas,
        recommendation,
        policy_reasons,
        measurement_gaps,
        input_refs,
        diagnostics,
    }
}

fn validate_run_set(
    diagnostics: &mut Vec<EvalDiagnostic>,
    arm_path: &str,
    runs: &[EvalRunInput],
    policy: &EvalComparePolicy,
) {
    if runs.is_empty() {
        diagnostics.push(EvalDiagnostic::error(
            EvalDiagnosticCode::EvalEmptyRunSet,
            format!("eval_compare_suite.{arm_path}.run_refs"),
            format!("{arm_path} has no eval run refs"),
        ));
        return;
    }
    if runs.len() < policy.minimum_task_count {
        diagnostics.push(EvalDiagnostic::error(
            EvalDiagnosticCode::EvalTaskCountBelowMinimum,
            format!("eval_compare_suite.{arm_path}.run_refs"),
            format!(
                "{arm_path} has {} run(s), below required minimum {}",
                runs.len(),
                policy.minimum_task_count
            ),
        ));
    }

    let mut seen = BTreeSet::new();
    for run in runs {
        if run.document.schema_version != EVAL_COMPARE_SCHEMA_VERSION {
            diagnostics.push(EvalDiagnostic::error(
                EvalDiagnosticCode::EvalUnsupportedRunSchemaVersion,
                run.source_ref.0.clone(),
                format!(
                    "eval run fixture has unsupported schema_version '{}', expected {EVAL_COMPARE_SCHEMA_VERSION}",
                    run.document.schema_version
                ),
            ));
        }
        let contract = &run.document.eval_run_contract;
        if !seen.insert(contract.task_id.0.clone()) {
            diagnostics.push(EvalDiagnostic::error(
                EvalDiagnosticCode::EvalDuplicateTaskRun,
                run.source_ref.0.clone(),
                format!("duplicate eval run for task_id {}", contract.task_id.0),
            ));
        }
        if policy.require_evidence_refs && contract.evidence_refs.is_empty() {
            diagnostics.push(EvalDiagnostic::error(
                EvalDiagnosticCode::EvalMissingEvidenceRefs,
                run.source_ref.0.clone(),
                format!("eval run {} has no evidence_refs", contract.run_id.0),
            ));
        }
        if policy.require_trace_refs && trace_refs(&contract.evidence_refs).is_empty() {
            diagnostics.push(EvalDiagnostic::error(
                EvalDiagnosticCode::EvalMissingTraceRefs,
                run.source_ref.0.clone(),
                format!(
                    "eval run {} has no trace-like evidence ref",
                    contract.run_id.0
                ),
            ));
        }
    }
}

fn validate_matching_tasks(
    diagnostics: &mut Vec<EvalDiagnostic>,
    baseline_runs: &[EvalRunInput],
    candidate_runs: &[EvalRunInput],
) {
    let baseline_tasks: BTreeSet<_> = baseline_runs
        .iter()
        .map(|run| run.document.eval_run_contract.task_id.0.clone())
        .collect();
    let candidate_tasks: BTreeSet<_> = candidate_runs
        .iter()
        .map(|run| run.document.eval_run_contract.task_id.0.clone())
        .collect();
    if baseline_tasks != candidate_tasks {
        diagnostics.push(EvalDiagnostic::error(
            EvalDiagnosticCode::EvalTaskSetMismatch,
            "eval_compare_suite.policy.require_matching_tasks",
            format!("baseline tasks {baseline_tasks:?} do not match candidate tasks {candidate_tasks:?}"),
        ));
    }
}

fn summarize_runs(label: EvalArmLabel, runs: &[EvalRunInput]) -> EvalArmSummary {
    let run_count = runs.len();
    let mut task_ids = BTreeSet::new();
    let mut successes = 0usize;
    let mut failures = 0usize;
    let mut partials = 0usize;
    let mut errors = 0usize;
    let mut total_cost_usd_micros = 0u64;
    let mut total_wall_time_ms = 0u64;
    let mut total_tool_calls = 0u64;
    let mut total_turns = 0u64;
    let mut total_tokens = 0u64;
    let mut failure_clusters = BTreeMap::new();
    let mut evidence_refs = BTreeSet::new();
    let mut trace_refs_set = BTreeSet::new();

    for run in runs {
        let contract = &run.document.eval_run_contract;
        task_ids.insert(contract.task_id.0.clone());
        match contract.outcome.value {
            EvalVerdict::Passed => successes += 1,
            EvalVerdict::Failed => failures += 1,
            EvalVerdict::Partial | EvalVerdict::Flaky => partials += 1,
            EvalVerdict::Error => errors += 1,
        }
        if let Some(cluster) = contract.outcome.failure_cluster {
            if cluster != EvalFailureCluster::None {
                *failure_clusters
                    .entry(serialized_failure_cluster(cluster))
                    .or_insert(0) += 1;
            }
        }
        total_cost_usd_micros =
            total_cost_usd_micros.saturating_add(contract.cost.estimated_cost_usd_micros);
        total_wall_time_ms = total_wall_time_ms.saturating_add(contract.cost.wall_time_ms);
        total_tool_calls = total_tool_calls.saturating_add(u64::from(contract.cost.num_tool_calls));
        total_turns = total_turns.saturating_add(u64::from(contract.cost.num_turns));
        total_tokens = total_tokens.saturating_add(contract.cost.total_tokens);
        for reference in &contract.evidence_refs {
            evidence_refs.insert(reference.clone());
        }
        for reference in trace_refs(&contract.evidence_refs) {
            trace_refs_set.insert(reference);
        }
    }

    EvalArmSummary {
        label,
        run_count,
        task_count: task_ids.len(),
        successes,
        failures,
        partials,
        errors,
        success_rate_bps: ratio_bps(successes, run_count),
        total_cost_usd_micros,
        mean_cost_usd_micros: mean_u64(total_cost_usd_micros, run_count),
        total_wall_time_ms,
        mean_wall_time_ms: mean_u64(total_wall_time_ms, run_count),
        total_tool_calls,
        mean_tool_calls_per_run_bps: mean_bps(total_tool_calls, run_count),
        total_turns,
        mean_turns_per_run_bps: mean_bps(total_turns, run_count),
        total_tokens,
        mean_tokens_per_run: mean_u64(total_tokens, run_count),
        failure_clusters,
        evidence_refs: evidence_refs.into_iter().collect(),
        trace_refs: trace_refs_set.into_iter().collect(),
    }
}

fn metric_deltas(baseline: &EvalArmSummary, candidate: &EvalArmSummary) -> EvalMetricDeltas {
    EvalMetricDeltas {
        success_rate_bps: i64::from(candidate.success_rate_bps)
            - i64::from(baseline.success_rate_bps),
        total_cost_usd_micros: i128::from(candidate.total_cost_usd_micros)
            - i128::from(baseline.total_cost_usd_micros),
        mean_cost_usd_micros: i128::from(candidate.mean_cost_usd_micros)
            - i128::from(baseline.mean_cost_usd_micros),
        total_wall_time_ms: i128::from(candidate.total_wall_time_ms)
            - i128::from(baseline.total_wall_time_ms),
        mean_wall_time_ms: i128::from(candidate.mean_wall_time_ms)
            - i128::from(baseline.mean_wall_time_ms),
        total_tool_calls: i128::from(candidate.total_tool_calls)
            - i128::from(baseline.total_tool_calls),
        total_turns: i128::from(candidate.total_turns) - i128::from(baseline.total_turns),
        total_tokens: i128::from(candidate.total_tokens) - i128::from(baseline.total_tokens),
    }
}

fn recommend_candidate(
    has_errors: bool,
    baseline: &EvalArmSummary,
    candidate: &EvalArmSummary,
    deltas: &EvalMetricDeltas,
) -> (EvalRecommendation, Vec<String>) {
    if has_errors {
        return (
            EvalRecommendation::BlockCandidate,
            vec!["comparison has blocking diagnostics".to_string()],
        );
    }
    if candidate.success_rate_bps < baseline.success_rate_bps {
        return (
            EvalRecommendation::KeepBaseline,
            vec!["candidate success rate is lower than baseline".to_string()],
        );
    }
    let quality_improved = candidate.success_rate_bps > baseline.success_rate_bps;
    let efficiency_improved = deltas.total_cost_usd_micros < 0
        || deltas.mean_wall_time_ms < 0
        || deltas.total_tool_calls < 0
        || deltas.total_turns < 0
        || deltas.total_tokens < 0;
    if quality_improved || efficiency_improved {
        return (
            EvalRecommendation::TryCandidate,
            vec!["candidate is not worse on success rate and improves at least one measured dimension".to_string()],
        );
    }
    if baseline.run_count == 0 || candidate.run_count == 0 {
        return (
            EvalRecommendation::Inconclusive,
            vec!["missing comparable runs".to_string()],
        );
    }
    (
        EvalRecommendation::KeepBaseline,
        vec![
            "candidate does not improve quality, cost, latency, tools, turns, or tokens"
                .to_string(),
        ],
    )
}

fn shared_task_count(baseline_runs: &[EvalRunInput], candidate_runs: &[EvalRunInput]) -> usize {
    let baseline_tasks: BTreeSet<_> = baseline_runs
        .iter()
        .map(|run| run.document.eval_run_contract.task_id.0.clone())
        .collect();
    candidate_runs
        .iter()
        .filter(|run| baseline_tasks.contains(&run.document.eval_run_contract.task_id.0))
        .count()
}

fn trace_refs(evidence_refs: &[String]) -> Vec<String> {
    evidence_refs
        .iter()
        .filter(|reference| {
            let lower = reference.to_ascii_lowercase();
            lower.starts_with("trace:") || lower.contains("trace") || lower.contains("traces/")
        })
        .cloned()
        .collect()
}

fn serialized_failure_cluster(cluster: EvalFailureCluster) -> String {
    match cluster {
        EvalFailureCluster::BuildFailure => "build_failure",
        EvalFailureCluster::TestFailure => "test_failure",
        EvalFailureCluster::LintFailure => "lint_failure",
        EvalFailureCluster::Timeout => "timeout",
        EvalFailureCluster::WrongLocation => "wrong_location",
        EvalFailureCluster::OverfitPatch => "overfit_patch",
        EvalFailureCluster::SemanticMismatch => "semantic_mismatch",
        EvalFailureCluster::ToolError => "tool_error",
        EvalFailureCluster::None => "none",
    }
    .to_string()
}

fn ratio_bps(numerator: usize, denominator: usize) -> u32 {
    numerator
        .saturating_mul(10_000)
        .checked_div(denominator)
        .map_or(0, |v| u32::try_from(v).unwrap_or(u32::MAX))
}

fn mean_u64(total: u64, count: usize) -> u64 {
    if count == 0 {
        0
    } else {
        total / u64::try_from(count).unwrap_or(u64::MAX)
    }
}

fn mean_bps(total: u64, count: usize) -> u64 {
    if count == 0 {
        0
    } else {
        total.saturating_mul(10_000) / u64::try_from(count).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core_contracts::eval_run::{EvalCost, EvalOutcome, QualitySignals};

    fn suite() -> EvalCompareSuite {
        EvalCompareSuite {
            id: StableId("eval.suite.smoke".to_string()),
            comparison_id: StableId("eval.compare.smoke".to_string()),
            baseline: EvalArmSpec {
                label: EvalArmLabel::SingleAgent,
                run_refs: Vec::new(),
            },
            candidate: EvalArmSpec {
                label: EvalArmLabel::Graph,
                run_refs: Vec::new(),
            },
            policy: EvalComparePolicy {
                require_matching_tasks: true,
                require_evidence_refs: true,
                require_trace_refs: true,
                minimum_task_count: 2,
            },
        }
    }

    #[derive(Clone, Copy)]
    struct RunFixture {
        verdict: EvalVerdict,
        cluster: Option<EvalFailureCluster>,
        cost: u64,
        wall_time: u64,
        tools: u32,
        turns: u32,
    }

    fn fixture(
        verdict: EvalVerdict,
        cluster: Option<EvalFailureCluster>,
        cost: u64,
        wall_time: u64,
        tools: u32,
        turns: u32,
    ) -> RunFixture {
        RunFixture {
            verdict,
            cluster,
            cost,
            wall_time,
            tools,
            turns,
        }
    }

    fn run(source: &str, run_id: &str, task_id: &str, fixture: RunFixture) -> EvalRunInput {
        EvalRunInput {
            source_ref: RepoPath(source.to_string()),
            document: EvalRunContractDocument {
                schema_version: "0.1".to_string(),
                eval_run_contract: forge_core_contracts::EvalRunContract {
                    run_id: StableId(run_id.to_string()),
                    task_id: StableId(task_id.to_string()),
                    model_ref: "fixture-model".to_string(),
                    router_decision: None,
                    outcome: EvalOutcome {
                        value: fixture.verdict,
                        evaluated_at: "2026-06-29T00:00:00Z".to_string(),
                        failure_cluster: fixture.cluster,
                        notes: None,
                    },
                    cost: EvalCost {
                        prompt_tokens: 100,
                        completion_tokens: 50,
                        total_tokens: 150,
                        estimated_cost_usd_micros: fixture.cost,
                        wall_time_ms: fixture.wall_time,
                        num_tool_calls: fixture.tools,
                        num_turns: fixture.turns,
                    },
                    quality_signals: QualitySignals {
                        correct_location: Some(true),
                        semantic_correct: Some(matches!(fixture.verdict, EvalVerdict::Passed)),
                        overfit_suspected: Some(false),
                        confidence: Some(80),
                    },
                    evidence_refs: vec![
                        format!("evidence/{run_id}.json"),
                        format!("traces/{run_id}.ndjson"),
                    ],
                },
            },
        }
    }

    #[test]
    fn compare_reports_success_cost_latency_and_failure_deltas() {
        let report = compare_eval_runs(
            &suite(),
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Graph,
            &[
                run(
                    "single/a.yaml",
                    "sa-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
                run(
                    "single/b.yaml",
                    "sa-b",
                    "task-b",
                    fixture(
                        EvalVerdict::Failed,
                        Some(EvalFailureCluster::TestFailure),
                        100,
                        1000,
                        4,
                        2,
                    ),
                ),
            ],
            &[
                run(
                    "graph/a.yaml",
                    "graph-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
                run(
                    "graph/b.yaml",
                    "graph-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
            ],
        );

        assert_eq!(report.status, EvalCompareStatus::Passed);
        assert_eq!(report.baseline_summary.success_rate_bps, 5_000);
        assert_eq!(report.candidate_summary.success_rate_bps, 10_000);
        assert_eq!(report.deltas.success_rate_bps, 5_000);
        assert_eq!(report.deltas.total_cost_usd_micros, -40);
        assert_eq!(report.deltas.total_wall_time_ms, -200);
        assert_eq!(report.recommendation, EvalRecommendation::TryCandidate);
        assert_eq!(
            report.baseline_summary.failure_clusters.get("test_failure"),
            Some(&1)
        );
    }

    #[test]
    fn candidate_with_lower_success_keeps_baseline() {
        let report = compare_eval_runs(
            &suite(),
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Graph,
            &[
                run(
                    "single/a.yaml",
                    "sa-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
                run(
                    "single/b.yaml",
                    "sa-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
            ],
            &[
                run(
                    "graph/a.yaml",
                    "graph-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
                run(
                    "graph/b.yaml",
                    "graph-b",
                    "task-b",
                    fixture(
                        EvalVerdict::Failed,
                        Some(EvalFailureCluster::SemanticMismatch),
                        80,
                        900,
                        3,
                        2,
                    ),
                ),
            ],
        );

        assert_eq!(report.status, EvalCompareStatus::Passed);
        assert_eq!(report.recommendation, EvalRecommendation::KeepBaseline);
    }

    #[test]
    fn mismatched_tasks_and_missing_trace_refs_block_candidate() {
        let mut candidate = run(
            "graph/c.yaml",
            "graph-c",
            "task-c",
            fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
        );
        candidate.document.eval_run_contract.evidence_refs =
            vec!["evidence/graph-c.json".to_string()];
        let report = compare_eval_runs(
            &suite(),
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Graph,
            &[
                run(
                    "single/a.yaml",
                    "sa-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
                run(
                    "single/b.yaml",
                    "sa-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
            ],
            &[
                run(
                    "graph/a.yaml",
                    "graph-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
                candidate,
            ],
        );

        assert_eq!(report.status, EvalCompareStatus::Blocked);
        assert_eq!(report.recommendation, EvalRecommendation::BlockCandidate);
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == EvalDiagnosticCode::EvalTaskSetMismatch));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == EvalDiagnosticCode::EvalMissingTraceRefs));
    }

    #[test]
    fn unsupported_run_schema_blocks_at_library_boundary() {
        let mut candidate = run(
            "graph/a.yaml",
            "graph-a",
            "task-a",
            fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
        );
        candidate.document.schema_version = "9.9".to_string();

        let report = compare_eval_runs(
            &suite(),
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Graph,
            &[
                run(
                    "single/a.yaml",
                    "sa-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
                run(
                    "single/b.yaml",
                    "sa-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
            ],
            &[
                candidate,
                run(
                    "graph/b.yaml",
                    "graph-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
            ],
        );

        assert_eq!(report.status, EvalCompareStatus::Blocked);
        assert_eq!(report.recommendation, EvalRecommendation::BlockCandidate);
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == EvalDiagnosticCode::EvalUnsupportedRunSchemaVersion
        }));
    }

    // ----- EvalArmLabel round-trip + aliases. as_str/Display/FromStr were
    // entirely untested before this commit.
    #[test]
    fn eval_arm_label_as_str_matches_canonical_kebab() {
        assert_eq!(EvalArmLabel::SingleAgent.as_str(), "single-agent");
        assert_eq!(EvalArmLabel::Graph.as_str(), "graph");
        assert_eq!(EvalArmLabel::Mas.as_str(), "mas");
        assert_eq!(EvalArmLabel::Manual.as_str(), "manual");
    }

    #[test]
    fn eval_arm_label_display_equals_as_str() {
        // Display delegates to as_str; pin both so they cannot drift apart.
        assert_eq!(format!("{}", EvalArmLabel::SingleAgent), "single-agent");
        assert_eq!(format!("{}", EvalArmLabel::Graph), "graph");
        assert_eq!(format!("{}", EvalArmLabel::Mas), "mas");
        assert_eq!(format!("{}", EvalArmLabel::Manual), "manual");
    }

    #[test]
    fn eval_arm_label_from_str_accepts_canonical_and_aliases() {
        // Canonical kebab.
        assert_eq!(
            EvalArmLabel::from_str("single-agent").unwrap(),
            EvalArmLabel::SingleAgent
        );
        assert_eq!(
            EvalArmLabel::from_str("graph").unwrap(),
            EvalArmLabel::Graph
        );
        assert_eq!(EvalArmLabel::from_str("mas").unwrap(), EvalArmLabel::Mas);
        assert_eq!(
            EvalArmLabel::from_str("manual").unwrap(),
            EvalArmLabel::Manual
        );
        // Documented aliases (snake_case / long forms).
        assert_eq!(
            EvalArmLabel::from_str("single_agent").unwrap(),
            EvalArmLabel::SingleAgent
        );
        assert_eq!(
            EvalArmLabel::from_str("singleagent").unwrap(),
            EvalArmLabel::SingleAgent
        );
        assert_eq!(
            EvalArmLabel::from_str("workflow-graph").unwrap(),
            EvalArmLabel::Graph
        );
        assert_eq!(
            EvalArmLabel::from_str("workflow_graph").unwrap(),
            EvalArmLabel::Graph
        );
        assert_eq!(
            EvalArmLabel::from_str("multi-agent").unwrap(),
            EvalArmLabel::Mas
        );
        assert_eq!(
            EvalArmLabel::from_str("multi_agent").unwrap(),
            EvalArmLabel::Mas
        );
        assert_eq!(
            EvalArmLabel::from_str("human").unwrap(),
            EvalArmLabel::Manual
        );
    }

    #[test]
    fn eval_arm_label_from_str_rejects_unknown_with_typed_error() {
        let err = EvalArmLabel::from_str("bogus-arm").expect_err("unknown arm must error");
        // The typed error preserves the offending value (no String-only loss).
        assert_eq!(err.value, "bogus-arm");
        // Display surfaces the accepted vocabulary so an operator can self-correct.
        let msg = err.to_string();
        assert!(
            msg.contains("bogus-arm"),
            "error message should echo the bad value: {msg}"
        );
        assert!(
            msg.contains("single-agent"),
            "error message should list accepted arms: {msg}"
        );
    }

    #[test]
    fn eval_arm_label_round_trips_through_str() {
        // Asymmetric alias map means from_str(as_str(label)) == label for every
        // canonical label (the canonical form is always an accepted input).
        for label in [
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Graph,
            EvalArmLabel::Mas,
            EvalArmLabel::Manual,
        ] {
            let parsed = EvalArmLabel::from_str(label.as_str()).expect("canonical round-trips");
            assert_eq!(parsed, label);
        }
    }

    // ----- compare_eval_runs_with_diagnostics: the extra_diagnostics path and
    // the label-mismatch diagnostics were untested (only the empty-extras
    // wrapper compare_eval_runs was exercised).
    #[test]
    fn compare_preserves_caller_supplied_boundary_diagnostics() {
        // A clean comparison that would otherwise pass; we inject an extra
        // boundary diagnostic and assert it lands in the report and blocks.
        let mut extra = EvalDiagnostic::warning(
            EvalDiagnosticCode::EvalMissingTraceRefs,
            "boundary/sigstore",
            "release bundle lacked a cluster attestation",
        );
        // Force it to an error so it flips the recommendation (warnings don't).
        extra.severity = EvalDiagnosticSeverity::Error;

        let report = compare_eval_runs_with_diagnostics(
            &suite(),
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Graph,
            &[
                run(
                    "single/a.yaml",
                    "sa-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
                run(
                    "single/b.yaml",
                    "sa-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 100, 1000, 4, 2),
                ),
            ],
            &[
                run(
                    "graph/a.yaml",
                    "graph-a",
                    "task-a",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
                run(
                    "graph/b.yaml",
                    "graph-b",
                    "task-b",
                    fixture(EvalVerdict::Passed, None, 80, 900, 3, 2),
                ),
            ],
            vec![extra],
        );

        assert_eq!(report.status, EvalCompareStatus::Blocked);
        assert!(
            report.diagnostics.iter().any(|d| {
                d.code == EvalDiagnosticCode::EvalMissingTraceRefs && d.path == "boundary/sigstore"
            }),
            "the caller-supplied diagnostic must be preserved verbatim, got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn compare_flags_baseline_label_mismatch() {
        // The suite's baseline is SingleAgent; request Mas instead. This must
        // emit EvalBaselineLabelMismatch (a path no prior test exercised).
        let report = compare_eval_runs(
            &suite(),
            EvalArmLabel::Mas, // wrong — does not match suite.baseline.label
            EvalArmLabel::Graph,
            &[],
            &[],
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == EvalDiagnosticCode::EvalBaselineLabelMismatch),
            "baseline label mismatch must be flagged, got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn compare_flags_candidate_label_mismatch() {
        let report = compare_eval_runs(
            &suite(),
            EvalArmLabel::SingleAgent,
            EvalArmLabel::Mas, // wrong — does not match suite.candidate.label
            &[],
            &[],
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == EvalDiagnosticCode::EvalCandidateLabelMismatch),
            "candidate label mismatch must be flagged, got {:?}",
            report.diagnostics
        );
    }
}
