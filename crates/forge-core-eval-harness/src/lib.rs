//! Eval harness config schema and validator.
//!
//! `EvalHarnessConfig` describes the *execution* of a comparison experiment:
//! which corpus of tasks to run, which arms to spawn (each as a subprocess),
//! where to write canonical `EvalRunContractDocument`s, and the policy gates.
//! It is distinct from `EvalCompareSuite` (in `forge-core-eval`), which
//! describes the *comparison* of precomputed runs.
//!
//! Control of the ADR-0002 invariants ("same loader, tools, answer contract,
//! usage accounting") is structural rather than field-by-field: all arms share
//! one `corpus_ref` (same tasks), one grader (inferred from the corpus), and
//! the harness is the sole producer of the canonical contract. The per-arm
//! `command` embeds whatever loader/tools that arm uses; keeping them
//! equivalent across arms is the operator's convention, enforced by review of
//! the config, not by a schema field.

use forge_core_contracts::eval_run::{EvalFailureCluster, EvalVerdict};
use forge_core_contracts::{EvalRunContractDocument, RepoPath, StableId};
use forge_core_eval::EvalArmLabel;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod executor;

pub use executor::execute_run;

pub const EVAL_HARNESS_SCHEMA_VERSION: &str = "eval-harness-v0";

/// Substituted in an arm's `command` with the absolute path to a per-task YAML
/// file the harness writes before spawning. The arm reads the task from there.
pub const TASK_FILE_PLACEHOLDER: &str = "{task_file}";

/// Substituted in an arm's `command` with the stable id of the current task.
pub const TASK_ID_PLACEHOLDER: &str = "{task_id}";

/// Substituted in an arm's `command` with the absolute path where the arm must
/// write its raw `{ output, usage }` report. The harness reads it back, grades,
/// and canonicalises the contract.
pub const OUTPUT_FILE_PLACEHOLDER: &str = "{output_file}";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalHarnessConfigDocument {
    pub schema_version: String,
    pub eval_harness_config: EvalHarnessConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalHarnessConfig {
    pub id: StableId,
    /// Path to the shared task corpus (e.g. `router-eval-corpus.yaml`). Every
    /// arm runs the same tasks; this is the primary ADR-0002 control.
    pub corpus_ref: RepoPath,
    /// Directory where the harness writes per-run canonical contracts and
    /// intermediate task/output files.
    pub run_dir: RepoPath,
    pub arms: Vec<EvalHarnessArm>,
    pub policy: EvalHarnessPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalHarnessArm {
    pub label: EvalArmLabel,
    /// Argv for the subprocess. Placeholders `{task_file}`, `{task_id}`,
    /// `{output_file}` are substituted per run.
    pub command: Vec<String>,
    /// Per-run wall-clock budget. `None` means unbounded (not recommended).
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalHarnessPolicy {
    pub minimum_task_count: usize,
    pub require_matching_tasks: bool,
}

// ---------------------------------------------------------------------------
// Diagnostics — migrated to the canonical `forge_core_validate` types (V2.B).
//
// `HarnessDiagnostic`, `HarnessDiagnosticSeverity`, and `HarnessDiagnosticCode`
// were near-identical clones of `Diagnostic` / `DiagnosticSeverity` /
// `DiagnosticCode`. They are now aliases for the canonical types: the
// harness-specific code variants live in `forge_core_validate::DiagnosticCode`
// (prefixed `Harness*`), each with an explicit `#[serde(rename)]` preserving
// the original snake_case wire string. The canonical enum also gained
// `PartialOrd, Ord` so this crate's deterministic `sort_by_key` on
// `(code, path)` keeps working unchanged.
// ---------------------------------------------------------------------------

/// Canonical diagnostic, re-exported so harness callers keep their existing
/// import path (`HarnessDiagnostic`) without touching call sites.
pub type HarnessDiagnostic = forge_core_validate::Diagnostic;
/// Canonical diagnostic severity.
pub type HarnessDiagnosticSeverity = forge_core_validate::DiagnosticSeverity;
/// Canonical diagnostic code (the harness-specific variants are `Harness*`-
/// prefixed members of the canonical enum, renamed to their original wire
/// strings).
pub type HarnessDiagnosticCode = forge_core_validate::DiagnosticCode;

/// Error returned when a config document cannot be parsed at all (malformed
/// YAML or structural mismatch). Semantic diagnostics come from
/// [`validate_harness_config`] instead, so validation is cumulative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseHarnessConfigError {
    pub message: String,
}

impl fmt::Display for ParseHarnessConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to parse eval harness config: {}",
            self.message
        )
    }
}

impl std::error::Error for ParseHarnessConfigError {}

impl ParseHarnessConfigError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Parses a YAML config document.
///
/// Structural only. Follow with [`validate_harness_config`] to collect
/// semantic diagnostics.
///
/// # Errors
///
/// Returns `ParseHarnessConfigError` when the YAML is malformed or does not
/// match the config document shape (`deny_unknown_fields` rejects unknown
/// keys). Semantic problems are not errors here; they surface as diagnostics
/// from [`validate_harness_config_document`].
pub fn parse_harness_config(
    yaml: &str,
) -> Result<EvalHarnessConfigDocument, ParseHarnessConfigError> {
    yaml_serde::from_str::<EvalHarnessConfigDocument>(yaml)
        .map_err(|error| ParseHarnessConfigError::new(error.to_string()))
}

/// Cumulatively validates a parsed config, returning typed diagnostics. Never
/// short-circuits: every problem is collected so the operator can fix the whole
/// config in one pass.
#[must_use]
pub fn validate_harness_config(config: &EvalHarnessConfig) -> Vec<HarnessDiagnostic> {
    let mut diagnostics = Vec::new();

    if config.arms.is_empty() {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::HarnessEmptyArms,
            "eval_harness_config.arms",
            "at least two arms are required for a comparison",
        ));
    } else if config.arms.len() < 2 {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::HarnessFewerThanTwoArms,
            "eval_harness_config.arms",
            "a comparison needs a baseline and a candidate arm",
        ));
    }

    let mut seen_labels = std::collections::BTreeSet::new();
    for (index, arm) in config.arms.iter().enumerate() {
        let path = format!("eval_harness_config.arms[{index}]");
        if !seen_labels.insert(arm.label) {
            diagnostics.push(HarnessDiagnostic::error(
                HarnessDiagnosticCode::HarnessDuplicateArmLabel,
                path.clone(),
                format!("arm label '{}' is not unique", arm.label),
            ));
        }
        if arm.command.is_empty()
            || arm
                .command
                .first()
                .is_some_and(|token| token.trim().is_empty())
        {
            diagnostics.push(HarnessDiagnostic::error(
                HarnessDiagnosticCode::HarnessArmCommandEmpty,
                format!("{path}.command"),
                "arm command must start with a non-empty program token",
            ));
        }
        if arm.timeout_ms == Some(0) {
            diagnostics.push(HarnessDiagnostic::error(
                HarnessDiagnosticCode::HarnessArmTimeoutZero,
                format!("{path}.timeout_ms"),
                "timeout_ms must be positive when set, or omitted for unbounded",
            ));
        }
        let uses_placeholder = arm.command.iter().any(|token| {
            token.contains(TASK_FILE_PLACEHOLDER)
                || token.contains(TASK_ID_PLACEHOLDER)
                || token.contains(OUTPUT_FILE_PLACEHOLDER)
        });
        if !uses_placeholder {
            diagnostics.push(HarnessDiagnostic::warning(
                HarnessDiagnosticCode::HarnessPlaceholderMissing,
                format!("{path}.command"),
                "command contains no {task_file}/{task_id}/{output_file} placeholder; \
                 the arm will receive no task input and write no raw output",
            ));
        }
    }

    if config.corpus_ref.0.is_empty() {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::HarnessEmptyCorpusRef,
            "eval_harness_config.corpus_ref",
            "corpus_ref must point at a task corpus",
        ));
    }
    if config.run_dir.0.is_empty() {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::HarnessEmptyRunDir,
            "eval_harness_config.run_dir",
            "run_dir must be set",
        ));
    }

    diagnostics.sort_by_key(|diagnostic| (diagnostic.code, diagnostic.path.clone()));
    diagnostics
}

/// Validates both the document wrapper (schema version) and the inner config.
/// Cumulative; never short-circuits.
#[must_use]
pub fn validate_harness_config_document(
    document: &EvalHarnessConfigDocument,
) -> Vec<HarnessDiagnostic> {
    let mut diagnostics = Vec::new();
    if document.schema_version != EVAL_HARNESS_SCHEMA_VERSION {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::HarnessUnsupportedSchemaVersion,
            "schema_version",
            format!(
                "expected '{}', got '{}'",
                EVAL_HARNESS_SCHEMA_VERSION, document.schema_version
            ),
        ));
    }
    diagnostics.extend(validate_harness_config(&document.eval_harness_config));
    diagnostics
}

// ===========================================================================
// Corpus + grader (pure half of the executor)
// ===========================================================================

/// How a task's verdict is computed. Inferred from the corpus shape, not
/// declared as a config field: router corpora map to `ExactMatch`,
/// coordination suites to `FixturePass`, and anything without an automatic
/// grader to `Manual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraderKind {
    ExactMatch,
    FixturePass,
    Manual,
}

/// Computes a verdict by comparing the arm's raw output against the expected
/// answer. Whitespace is trimmed; the comparison is otherwise exact so that
/// kebab-case workflow ids are not silently coerced.
#[must_use]
pub fn grade_output(grader: GraderKind, actual: &str, expected: &str) -> EvalVerdict {
    match grader {
        GraderKind::ExactMatch => {
            if actual.trim() == expected.trim() {
                EvalVerdict::Passed
            } else {
                EvalVerdict::Failed
            }
        }
        GraderKind::FixturePass | GraderKind::Manual => EvalVerdict::Error,
    }
}

/// One normalised task drawn from any supported corpus. The harness runs every
/// arm against the same set of tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalTask {
    pub task_id: StableId,
    pub input: String,
    pub expected: String,
    pub grader_kind: GraderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadCorpusError {
    pub message: String,
}

impl fmt::Display for LoadCorpusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "failed to load eval corpus: {}", self.message)
    }
}

impl std::error::Error for LoadCorpusError {}

impl LoadCorpusError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RouterCorpusDocument {
    eval_corpus: Vec<RouterCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RouterCase {
    utterance: String,
    expected_workflow: String,
    #[allow(dead_code)]
    phase: String,
}

/// Loads a router eval corpus (`utterance -> expected_workflow` cases) into
/// normalised tasks with the `ExactMatch` grader. Task ids are derived from the
/// phase and zero-padded index so they are stable across runs.
///
/// # Errors
///
/// Returns `LoadCorpusError` when the YAML is malformed or lacks the
/// `eval_corpus` array.
pub fn load_router_corpus(yaml: &str) -> Result<Vec<EvalTask>, LoadCorpusError> {
    let document = yaml_serde::from_str::<RouterCorpusDocument>(yaml)
        .map_err(|error| LoadCorpusError::new(error.to_string()))?;
    let tasks = document
        .eval_corpus
        .into_iter()
        .enumerate()
        .map(|(index, case)| EvalTask {
            task_id: StableId(format!("router-eval-{index:03}")),
            input: case.utterance,
            expected: case.expected_workflow,
            grader_kind: GraderKind::ExactMatch,
        })
        .collect();
    Ok(tasks)
}

/// Self-reported usage an arm writes alongside its raw output. `wall_time_ms`
/// is intentionally absent: the harness measures wall time externally, since
/// the arm cannot see its own spawn overhead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawUsage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub estimated_cost_usd_micros: u64,
    #[serde(default)]
    pub num_tool_calls: u32,
    #[serde(default)]
    pub num_turns: u32,
}

/// The raw JSON report an arm writes at `{output_file}`. The harness reads it,
/// applies the grader, and canonicalises the `EvalRunContractDocument`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawArmReport {
    pub output: String,
    #[serde(default)]
    pub model_ref: Option<String>,
    #[serde(default)]
    pub usage: RawUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRawReportError {
    pub message: String,
}

impl fmt::Display for ParseRawReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to parse arm raw report: {}",
            self.message
        )
    }
}

impl std::error::Error for ParseRawReportError {}

impl ParseRawReportError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Parses the JSON an arm wrote at `{output_file}`.
///
/// # Errors
///
/// Returns `ParseRawReportError` when the file is missing or is not valid JSON
/// matching `RawArmReport`.
pub fn parse_raw_report(json: &str) -> Result<RawArmReport, ParseRawReportError> {
    serde_json::from_str::<RawArmReport>(json)
        .map_err(|error| ParseRawReportError::new(error.to_string()))
}

// ===========================================================================
// Canonicalisation + argv preparation (pure, spawn-free)
// ===========================================================================

/// Substitutes `{task_file}`, `{task_id}`, `{output_file}` placeholders in an
/// arm's command argv. Tokens that contain no placeholder are returned
/// unchanged. The resulting argv is what the subprocess executor spawns.
#[must_use]
pub fn substitute_placeholders(
    command: &[String],
    task_file: &str,
    task_id: &str,
    output_file: &str,
) -> Vec<String> {
    command
        .iter()
        .map(|token| {
            token
                .replace(TASK_FILE_PLACEHOLDER, task_file)
                .replace(TASK_ID_PLACEHOLDER, task_id)
                .replace(OUTPUT_FILE_PLACEHOLDER, output_file)
        })
        .collect()
}

/// Builds the canonical `EvalRunContractDocument` for one (arm, task) run. The
/// harness — not the arm — is the sole producer of this document: the verdict
/// comes from [`grade_output`], cost comes from the arm's self-reported usage
/// plus the harness-measured wall time. Run id is stable because the design is
    /// one run per (arm, task) (F05 eval-harness design, decision 4; see
    /// `dev-journals/f05_eval_harness_design.md` in the Forge-method-archive
    /// sibling repo).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_run_contract(
    arm_label: EvalArmLabel,
    task: &EvalTask,
    report: &RawArmReport,
    wall_time_ms: u64,
    evaluated_at: &str,
    raw_report_path: Option<&str>,
) -> EvalRunContractDocument {
    let verdict = grade_output(task.grader_kind, &report.output, &task.expected);
    let failure_cluster = match verdict {
        EvalVerdict::Failed => Some(EvalFailureCluster::SemanticMismatch),
        _ => None,
    };
    let notes = match task.grader_kind {
        GraderKind::ExactMatch => None,
        GraderKind::FixturePass => {
            Some("fixture-pass grader not yet implemented; verdict deferred".to_string())
        }
        GraderKind::Manual => {
            Some("manual grader: verdict deferred until human review".to_string())
        }
    };
    let model_ref = report
        .model_ref
        .clone()
        .unwrap_or_else(|| format!("harness-arm:{arm_label}"));
    let evidence_refs = raw_report_path
        .map(path_to_evidence_ref)
        .into_iter()
        .collect();
    let usage = &report.usage;
    EvalRunContractDocument {
        // EvalRunContractDocument schema version is "0.1" (see
        // forge-core-contracts/src/eval_run.rs); no public const is exported.
        schema_version: "0.1".to_string(),
        eval_run_contract: forge_core_contracts::eval_run::EvalRunContract {
            run_id: StableId(format!("eval.run.{}.{}", task.task_id.0, arm_label)),
            task_id: StableId(task.task_id.0.clone()),
            model_ref,
            router_decision: None,
            outcome: forge_core_contracts::eval_run::EvalOutcome {
                value: verdict,
                evaluated_at: evaluated_at.to_string(),
                failure_cluster,
                notes,
            },
            cost: forge_core_contracts::eval_run::EvalCost {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
                estimated_cost_usd_micros: usage.estimated_cost_usd_micros,
                wall_time_ms,
                num_tool_calls: usage.num_tool_calls,
                num_turns: usage.num_turns,
            },
            quality_signals: forge_core_contracts::eval_run::QualitySignals {
                correct_location: None,
                semantic_correct: None,
                overfit_suspected: None,
                confidence: None,
            },
            evidence_refs,
        },
    }
}

fn path_to_evidence_ref(path: &str) -> String {
    format!("raw:{path}")
}

/// Builds a contract for a run the harness could not complete (spawn failure,
/// nonzero exit, missing/invalid output file, timeout). The verdict is always
/// `Error` so the run is counted but never silently dropped -- the comparison
/// needs one document per (arm, task).
#[must_use]
pub fn build_error_contract(
    arm_label: EvalArmLabel,
    task: &EvalTask,
    failure_cluster: EvalFailureCluster,
    notes: impl Into<String>,
    evaluated_at: &str,
) -> EvalRunContractDocument {
    EvalRunContractDocument {
        // EvalRunContractDocument schema version is "0.1" (see
        // forge-core-contracts/src/eval_run.rs); no public const is exported.
        schema_version: "0.1".to_string(),
        eval_run_contract: forge_core_contracts::eval_run::EvalRunContract {
            run_id: StableId(format!("eval.run.{}.{}", task.task_id.0, arm_label)),
            task_id: StableId(task.task_id.0.clone()),
            model_ref: format!("harness-arm:{arm_label}"),
            router_decision: None,
            outcome: forge_core_contracts::eval_run::EvalOutcome {
                value: EvalVerdict::Error,
                evaluated_at: evaluated_at.to_string(),
                failure_cluster: Some(failure_cluster),
                notes: Some(notes.into()),
            },
            cost: forge_core_contracts::eval_run::EvalCost {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                estimated_cost_usd_micros: 0,
                wall_time_ms: 0,
                num_tool_calls: 0,
                num_turns: 0,
            },
            quality_signals: forge_core_contracts::eval_run::QualitySignals {
                correct_location: None,
                semantic_correct: None,
                overfit_suspected: None,
                confidence: None,
            },
            evidence_refs: Vec::new(),
        },
    }
}

// ===========================================================================
// Report generator (F05.4): bridges harness config -> comparison suite
// ===========================================================================

use forge_core_eval::{
    compare_eval_runs, EvalArmSpec, EvalComparePolicy, EvalCompareSuite, EvalComparisonReport,
    EvalRunInput,
};

/// Builds the `EvalCompareSuite` consumed by `compare_eval_runs` from a harness
/// config. The first arm is the baseline (ADR-0002 anchor), the second is the
/// candidate. `require_evidence_refs` defaults true because the harness emits a
/// raw-report evidence ref for every run; `require_trace_refs` defaults false
/// until F05.6 wires trace into the contracts.
#[must_use]
pub fn build_compare_suite(config: &EvalHarnessConfig) -> EvalCompareSuite {
    let baseline = &config.arms[0];
    let candidate = &config.arms[1];
    EvalCompareSuite {
        id: StableId(config.id.0.clone()),
        comparison_id: StableId(config.id.0.clone()),
        baseline: EvalArmSpec {
            label: baseline.label,
            run_refs: vec![RepoPath(format!("{}/{}", config.run_dir.0, baseline.label))],
        },
        candidate: EvalArmSpec {
            label: candidate.label,
            run_refs: vec![RepoPath(format!(
                "{}/{}",
                config.run_dir.0, candidate.label
            ))],
        },
        policy: EvalComparePolicy {
            require_matching_tasks: config.policy.require_matching_tasks,
            require_evidence_refs: true,
            require_trace_refs: false,
            minimum_task_count: config.policy.minimum_task_count,
        },
    }
}

/// Wraps each canonical contract document in the `EvalRunInput` shape
/// `compare_eval_runs` expects, pointing `source_ref` at the per-arm run
/// directory under `run_dir`.
#[must_use]
pub fn to_run_inputs(
    documents: &[EvalRunContractDocument],
    arm_label: EvalArmLabel,
    run_dir: &str,
) -> Vec<EvalRunInput> {
    let source_ref = RepoPath(format!("{run_dir}/{arm_label}"));
    documents
        .iter()
        .map(|document| EvalRunInput {
            source_ref: source_ref.clone(),
            document: document.clone(),
        })
        .collect()
}

/// Runs the full comparison: builds the suite from the config, wraps the
/// per-arm canonical contracts as run inputs, and delegates to
/// `compare_eval_runs`. The harness is the sole caller of this entry point --
/// it produced every document it passes in, so the comparison is over
/// consistently graded, uniformly canonicalised runs.
#[must_use]
pub fn generate_comparison_report(
    config: &EvalHarnessConfig,
    baseline_documents: &[EvalRunContractDocument],
    candidate_documents: &[EvalRunContractDocument],
) -> EvalComparisonReport {
    let suite = build_compare_suite(config);
    let baseline_runs = to_run_inputs(baseline_documents, suite.baseline.label, &config.run_dir.0);
    let candidate_runs = to_run_inputs(
        candidate_documents,
        suite.candidate.label,
        &config.run_dir.0,
    );
    compare_eval_runs(
        &suite,
        suite.baseline.label,
        suite.candidate.label,
        &baseline_runs,
        &candidate_runs,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::pedantic)]

    use super::*;

    const VALID_YAML: &str = "\
schema_version: \"eval-harness-v0\"
eval_harness_config:
  id: \"eval.harness.router_compare\"
  corpus_ref: \"contracts/eval/router-eval-corpus.yaml\"
  run_dir: \"target/eval-runs/router_compare\"
  arms:
    - label: \"single-agent\"
      command: [\"agent\", \"--input\", \"{task_file}\", \"--output\", \"{output_file}\"]
      timeout_ms: 60000
    - label: \"mas\"
      command: [\"mas-runner\", \"--task\", \"{task_id}\", \"--out\", \"{output_file}\"]
  policy:
    minimum_task_count: 3
    require_matching_tasks: true
";

    #[test]
    fn parses_valid_config() {
        let document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        assert_eq!(document.schema_version, EVAL_HARNESS_SCHEMA_VERSION);
        assert_eq!(document.eval_harness_config.arms.len(), 2);
    }

    #[test]
    fn clean_config_validates_without_diagnostics() {
        let document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        let diagnostics = validate_harness_config_document(&document);
        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics, got {diagnostics:?}"
        );
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.schema_version = "eval-harness-v999".to_string();
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics.iter().any(|diagnostic| {
            diagnostic.code == HarnessDiagnosticCode::HarnessUnsupportedSchemaVersion
        });
        assert!(
            has,
            "expected UnsupportedHarnessSchemaVersion, got {diagnostics:?}"
        );
    }

    #[test]
    fn flags_fewer_than_two_arms() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms.truncate(1);
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::HarnessFewerThanTwoArms);
        assert!(has, "expected FewerThanTwoArms, got {diagnostics:?}");
    }

    #[test]
    fn flags_duplicate_arm_labels() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[1].label = document.eval_harness_config.arms[0].label;
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::HarnessDuplicateArmLabel);
        assert!(has, "expected DuplicateArmLabel, got {diagnostics:?}");
    }

    #[test]
    fn warns_when_no_placeholder_present() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[0].command = vec!["static-bin".to_string()];
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::HarnessPlaceholderMissing);
        assert!(
            has,
            "expected PlaceholderMissing warning, got {diagnostics:?}"
        );
    }

    #[test]
    fn flags_zero_timeout() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[0].timeout_ms = Some(0);
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::HarnessArmTimeoutZero);
        assert!(has, "expected ArmTimeoutZero, got {diagnostics:?}");
    }

    #[test]
    fn empty_command_is_an_error() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[0].command = Vec::new();
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::HarnessArmCommandEmpty);
        assert!(has, "expected ArmCommandEmpty, got {diagnostics:?}");
    }

    #[test]
    fn malformed_yaml_is_a_parse_error() {
        let result = parse_harness_config("this: [is not: valid");
        assert!(result.is_err(), "expected parse error for malformed yaml");
    }

    #[test]
    fn on_disk_valid_fixture_validates_clean() {
        let yaml = include_str!("../fixtures/valid-router-compare.yaml");
        let document = parse_harness_config(yaml).expect("valid fixture should parse");
        let diagnostics = validate_harness_config_document(&document);
        assert!(
            diagnostics.is_empty(),
            "on-disk valid fixture must be clean, got {diagnostics:?}"
        );
    }

    #[test]
    fn on_disk_invalid_fixture_flags_duplicate_labels() {
        let yaml = include_str!("../fixtures/invalid-duplicate-labels.yaml");
        let document = parse_harness_config(yaml).expect("invalid fixture should still parse");
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::HarnessDuplicateArmLabel);
        assert!(
            has,
            "on-disk invalid fixture must flag DuplicateArmLabel, got {diagnostics:?}"
        );
    }

    #[test]
    fn grader_exact_match_passes_on_equal_output() {
        assert_eq!(
            grade_output(GraderKind::ExactMatch, "discover-intent", "discover-intent"),
            EvalVerdict::Passed
        );
    }

    #[test]
    fn grader_exact_match_trims_whitespace() {
        assert_eq!(
            grade_output(
                GraderKind::ExactMatch,
                "  discover-intent\n",
                "discover-intent"
            ),
            EvalVerdict::Passed
        );
    }

    #[test]
    fn grader_exact_match_fails_on_mismatch() {
        assert_eq!(
            grade_output(GraderKind::ExactMatch, "brainstorming", "discover-intent"),
            EvalVerdict::Failed
        );
    }

    #[test]
    fn grader_manual_and_fixture_yield_error_until_human_review() {
        assert_eq!(
            grade_output(GraderKind::Manual, "x", "y"),
            EvalVerdict::Error
        );
        assert_eq!(
            grade_output(GraderKind::FixturePass, "x", "y"),
            EvalVerdict::Error
        );
    }

    #[test]
    fn router_corpus_loads_into_exact_match_tasks() {
        let yaml = "\
eval_corpus:
  - utterance: \"help me brainstorm\"
    expected_workflow: brainstorming
    phase: 1-discovery
  - utterance: \"write the prd\"
    expected_workflow: write-spec
    phase: 2-specification
";
        let tasks = load_router_corpus(yaml).expect("router corpus should load");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].task_id.0, "router-eval-000");
        assert_eq!(tasks[0].expected, "brainstorming");
        assert_eq!(tasks[0].grader_kind, GraderKind::ExactMatch);
        assert_eq!(tasks[1].task_id.0, "router-eval-001");
    }

    #[test]
    fn router_corpus_rejects_malformed_yaml() {
        let result = load_router_corpus("eval_corpus: [this is broken");
        assert!(result.is_err(), "expected load error for malformed corpus");
    }

    #[test]
    fn raw_report_parses_full_json() {
        let json = r#"{"output":"discover-intent","usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15,"estimated_cost_usd_micros":200,"num_tool_calls":2,"num_turns":1}}"#;
        let report = parse_raw_report(json).expect("valid report should parse");
        assert_eq!(report.output, "discover-intent");
        assert_eq!(report.usage.total_tokens, 15);
        assert_eq!(report.usage.num_tool_calls, 2);
    }

    #[test]
    fn raw_report_parses_with_default_usage() {
        let json = r#"{"output":"brainstorming"}"#;
        let report = parse_raw_report(json).expect("report without usage should parse");
        assert_eq!(report.output, "brainstorming");
        assert_eq!(report.usage.total_tokens, 0);
    }

    #[test]
    fn raw_report_rejects_unknown_fields() {
        let json = r#"{"output":"x","surprise":true}"#;
        assert!(parse_raw_report(json).is_err());
    }

    #[test]
    fn substitute_replaces_all_placeholders() {
        let command = vec![
            "agent".to_string(),
            "--input".to_string(),
            "{task_file}".to_string(),
            "--tag".to_string(),
            "{task_id}".to_string(),
            "--out".to_string(),
            "{output_file}".to_string(),
        ];
        let out = substitute_placeholders(
            &command,
            "/tmp/task.yaml",
            "router-eval-000",
            "/tmp/out.json",
        );
        assert_eq!(out[2], "/tmp/task.yaml");
        assert_eq!(out[4], "router-eval-000");
        assert_eq!(out[6], "/tmp/out.json");
        assert_eq!(out[0], "agent");
    }

    #[test]
    fn substitute_passes_through_tokens_without_placeholders() {
        let command = vec!["static-bin".to_string(), "--flag".to_string()];
        let out = substitute_placeholders(&command, "a", "b", "c");
        assert_eq!(out, vec!["static-bin".to_string(), "--flag".to_string()]);
    }

    fn router_task(expected: &str) -> EvalTask {
        EvalTask {
            task_id: StableId("router-eval-000".to_string()),
            input: "help me brainstorm".to_string(),
            expected: expected.to_string(),
            grader_kind: GraderKind::ExactMatch,
        }
    }

    #[test]
    fn build_contract_marks_passed_run_with_matching_output() {
        let task = router_task("brainstorming");
        let report = RawArmReport {
            output: "brainstorming".to_string(),
            model_ref: Some("openai/gpt-5.5".to_string()),
            usage: RawUsage {
                prompt_tokens: 8_000,
                completion_tokens: 2_000,
                total_tokens: 10_000,
                estimated_cost_usd_micros: 125_000,
                num_tool_calls: 12,
                num_turns: 3,
            },
        };
        let document = build_run_contract(
            EvalArmLabel::SingleAgent,
            &task,
            &report,
            95_000,
            "2026-07-01T00:00:00Z",
            Some("/tmp/out.json"),
        );
        let contract = &document.eval_run_contract;
        assert_eq!(contract.outcome.value, EvalVerdict::Passed);
        assert_eq!(contract.outcome.failure_cluster, None);
        assert_eq!(contract.model_ref, "openai/gpt-5.5");
        assert_eq!(contract.cost.wall_time_ms, 95_000);
        assert_eq!(contract.cost.total_tokens, 10_000);
        assert_eq!(
            contract.evidence_refs,
            vec!["raw:/tmp/out.json".to_string()]
        );
        assert_eq!(contract.run_id.0, "eval.run.router-eval-000.single-agent");
    }

    #[test]
    fn build_contract_marks_failed_run_with_semantic_mismatch() {
        let task = router_task("brainstorming");
        let report = RawArmReport {
            output: "write-spec".to_string(),
            model_ref: None,
            usage: RawUsage::default(),
        };
        let document = build_run_contract(
            EvalArmLabel::Mas,
            &task,
            &report,
            1_000,
            "2026-07-01T00:00:00Z",
            None,
        );
        assert_eq!(
            document.eval_run_contract.outcome.value,
            EvalVerdict::Failed
        );
        assert_eq!(
            document.eval_run_contract.outcome.failure_cluster,
            Some(EvalFailureCluster::SemanticMismatch)
        );
        // Fallback model_ref when the arm did not self-report one.
        assert_eq!(document.eval_run_contract.model_ref, "harness-arm:mas");
        assert!(document.eval_run_contract.evidence_refs.is_empty());
    }

    fn two_arm_config() -> EvalHarnessConfig {
        parse_harness_config(include_str!("../fixtures/valid-router-compare.yaml"))
            .expect("valid fixture parses")
            .eval_harness_config
    }

    fn contract(arm: EvalArmLabel, task_id: &str, verdict: EvalVerdict) -> EvalRunContractDocument {
        EvalRunContractDocument {
            schema_version: "0.1".to_string(),
            eval_run_contract: forge_core_contracts::eval_run::EvalRunContract {
                run_id: StableId(format!("eval.run.{}.{}", task_id, arm)),
                task_id: StableId(task_id.to_string()),
                model_ref: format!("harness-arm:{}", arm),
                router_decision: None,
                outcome: forge_core_contracts::eval_run::EvalOutcome {
                    value: verdict,
                    evaluated_at: "2026-07-01T00:00:00Z".to_string(),
                    failure_cluster: None,
                    notes: None,
                },
                cost: forge_core_contracts::eval_run::EvalCost {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                    estimated_cost_usd_micros: 1_000,
                    wall_time_ms: 5_000,
                    num_tool_calls: 2,
                    num_turns: 1,
                },
                quality_signals: forge_core_contracts::eval_run::QualitySignals {
                    correct_location: None,
                    semantic_correct: None,
                    overfit_suspected: None,
                    confidence: None,
                },
                evidence_refs: vec!["raw:/tmp/x".to_string()],
            },
        }
    }

    #[test]
    fn report_keeps_baseline_when_candidate_does_not_beat_it() {
        let config = two_arm_config();
        // Baseline passes both tasks; candidate passes one, fails one.
        let baseline = vec![
            contract(
                EvalArmLabel::SingleAgent,
                "router-eval-000",
                EvalVerdict::Passed,
            ),
            contract(
                EvalArmLabel::SingleAgent,
                "router-eval-001",
                EvalVerdict::Passed,
            ),
        ];
        let candidate = vec![
            contract(EvalArmLabel::Mas, "router-eval-000", EvalVerdict::Passed),
            contract(EvalArmLabel::Mas, "router-eval-001", EvalVerdict::Failed),
        ];
        let report = generate_comparison_report(&config, &baseline, &candidate);
        assert_eq!(report.baseline, EvalArmLabel::SingleAgent);
        assert_eq!(report.candidate, EvalArmLabel::Mas);
        assert_eq!(report.baseline_summary.successes, 2);
        assert_eq!(report.candidate_summary.successes, 1);
    }

    #[test]
    fn to_run_inputs_sets_per_arm_source_ref() {
        let docs = vec![contract(
            EvalArmLabel::SingleAgent,
            "router-eval-000",
            EvalVerdict::Passed,
        )];
        let inputs = to_run_inputs(&docs, EvalArmLabel::SingleAgent, "target/eval-runs/x");
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].source_ref.0, "target/eval-runs/x/single-agent");
    }

    #[test]
    fn build_suite_marks_baseline_and_candidate_from_first_two_arms() {
        let config = two_arm_config();
        let suite = build_compare_suite(&config);
        assert_eq!(suite.baseline.label, EvalArmLabel::SingleAgent);
        assert_eq!(suite.candidate.label, EvalArmLabel::Mas);
        assert!(suite.policy.require_evidence_refs);
        assert!(!suite.policy.require_trace_refs);
    }
}
