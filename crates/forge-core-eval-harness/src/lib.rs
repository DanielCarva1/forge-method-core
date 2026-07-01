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

use forge_core_contracts::{RepoPath, StableId};
use forge_core_eval::EvalArmLabel;
use serde::{Deserialize, Serialize};
use std::fmt;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessDiagnosticCode {
    UnsupportedHarnessSchemaVersion,
    EmptyArms,
    FewerThanTwoArms,
    DuplicateArmLabel,
    ArmCommandEmpty,
    ArmTimeoutZero,
    EmptyCorpusRef,
    EmptyRunDir,
    PlaceholderMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessDiagnostic {
    pub severity: HarnessDiagnosticSeverity,
    pub code: HarnessDiagnosticCode,
    pub path: String,
    pub message: String,
}

impl HarnessDiagnostic {
    #[must_use]
    pub fn error(
        code: HarnessDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: HarnessDiagnosticSeverity::Error,
            code,
            path: path.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn warning(
        code: HarnessDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: HarnessDiagnosticSeverity::Warning,
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

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
            HarnessDiagnosticCode::EmptyArms,
            "eval_harness_config.arms",
            "at least two arms are required for a comparison",
        ));
    } else if config.arms.len() < 2 {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::FewerThanTwoArms,
            "eval_harness_config.arms",
            "a comparison needs a baseline and a candidate arm",
        ));
    }

    let mut seen_labels = std::collections::BTreeSet::new();
    for (index, arm) in config.arms.iter().enumerate() {
        let path = format!("eval_harness_config.arms[{index}]");
        if !seen_labels.insert(arm.label) {
            diagnostics.push(HarnessDiagnostic::error(
                HarnessDiagnosticCode::DuplicateArmLabel,
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
                HarnessDiagnosticCode::ArmCommandEmpty,
                format!("{path}.command"),
                "arm command must start with a non-empty program token",
            ));
        }
        if arm.timeout_ms == Some(0) {
            diagnostics.push(HarnessDiagnostic::error(
                HarnessDiagnosticCode::ArmTimeoutZero,
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
                HarnessDiagnosticCode::PlaceholderMissing,
                format!("{path}.command"),
                "command contains no {task_file}/{task_id}/{output_file} placeholder; \
                 the arm will receive no task input and write no raw output",
            ));
        }
    }

    if config.corpus_ref.0.is_empty() {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::EmptyCorpusRef,
            "eval_harness_config.corpus_ref",
            "corpus_ref must point at a task corpus",
        ));
    }
    if config.run_dir.0.is_empty() {
        diagnostics.push(HarnessDiagnostic::error(
            HarnessDiagnosticCode::EmptyRunDir,
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
            HarnessDiagnosticCode::UnsupportedHarnessSchemaVersion,
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
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::UnsupportedHarnessSchemaVersion);
        assert!(has, "expected UnsupportedHarnessSchemaVersion, got {diagnostics:?}");
    }

    #[test]
    fn flags_fewer_than_two_arms() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms.truncate(1);
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::FewerThanTwoArms);
        assert!(has, "expected FewerThanTwoArms, got {diagnostics:?}");
    }

    #[test]
    fn flags_duplicate_arm_labels() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[1].label = document.eval_harness_config.arms[0].label;
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::DuplicateArmLabel);
        assert!(has, "expected DuplicateArmLabel, got {diagnostics:?}");
    }

    #[test]
    fn warns_when_no_placeholder_present() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[0].command = vec!["static-bin".to_string()];
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::PlaceholderMissing);
        assert!(has, "expected PlaceholderMissing warning, got {diagnostics:?}");
    }

    #[test]
    fn flags_zero_timeout() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[0].timeout_ms = Some(0);
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::ArmTimeoutZero);
        assert!(has, "expected ArmTimeoutZero, got {diagnostics:?}");
    }

    #[test]
    fn empty_command_is_an_error() {
        let mut document = parse_harness_config(VALID_YAML).expect("valid yaml should parse");
        document.eval_harness_config.arms[0].command = Vec::new();
        let diagnostics = validate_harness_config_document(&document);
        let has = diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::ArmCommandEmpty);
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
            .any(|diagnostic| diagnostic.code == HarnessDiagnosticCode::DuplicateArmLabel);
        assert!(
            has,
            "on-disk invalid fixture must flag DuplicateArmLabel, got {diagnostics:?}"
        );
    }
}
