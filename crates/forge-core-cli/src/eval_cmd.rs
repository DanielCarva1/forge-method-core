use crate::cli_error::ExitError;
use crate::cli_util::eval_usage;
use crate::project_cmd::{resolve_project, ProjectResolveError};
use forge_core_contracts::{EvalRunContractDocument, RepoPath};
use forge_core_eval::{
    compare_eval_runs_with_diagnostics, EvalArmLabel, EvalCompareStatus, EvalCompareSuiteDocument,
    EvalComparisonReport, EvalDiagnostic, EvalDiagnosticCode, EvalRunInput,
};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_EVAL_COMPARE_SUITE: &str =
    "docs/fixtures/eval-run-v0/eval-compare-smoke-suite.yaml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalCompareCommandInput {
    pub root: PathBuf,
    pub suite_path: Option<PathBuf>,
    pub baseline: EvalArmLabel,
    pub candidate: EvalArmLabel,
    pub allow_bootstrap_core: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalCommandError {
    ProjectResolve(ProjectResolveError),
    CanonicalizeProjectRoot { path: PathBuf, source: String },
    ReadSuite { path: PathBuf, source: String },
    ParseSuite { path: PathBuf, source: String },
    UnsupportedSuiteSchemaVersion { path: PathBuf, found: String },
    ReadRun { path: PathBuf, source: String },
    ParseRun { path: PathBuf, source: String },
    UnsupportedRunSchemaVersion { path: PathBuf, found: String },
    InvalidSuitePath { path: String },
    InvalidRunPath { path: String },
}

impl fmt::Display for EvalCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectResolve(error) => write!(formatter, "project resolve failed: {error}"),
            Self::CanonicalizeProjectRoot { path, source } => write!(
                formatter,
                "canonicalize project root {} failed: {source}",
                path.display()
            ),
            Self::ReadSuite { path, source } => {
                write!(
                    formatter,
                    "read eval suite {} failed: {source}",
                    path.display()
                )
            }
            Self::ParseSuite { path, source } => {
                write!(
                    formatter,
                    "parse eval suite {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsupportedSuiteSchemaVersion { path, found } => write!(
                formatter,
                "eval suite {} has unsupported schema_version '{}', expected 0.1",
                path.display(),
                found
            ),
            Self::ReadRun { path, source } => {
                write!(
                    formatter,
                    "read eval run {} failed: {source}",
                    path.display()
                )
            }
            Self::ParseRun { path, source } => {
                write!(
                    formatter,
                    "parse eval run {} failed: {source}",
                    path.display()
                )
            }
            Self::UnsupportedRunSchemaVersion { path, found } => write!(
                formatter,
                "eval run {} has unsupported schema_version '{}', expected 0.1",
                path.display(),
                found
            ),
            Self::InvalidSuitePath { path } => write!(
                formatter,
                "eval suite path '{path}' is invalid; suite refs must stay under the project root"
            ),
            Self::InvalidRunPath { path } => write!(
                formatter,
                "eval run ref '{path}' is invalid; refs must stay under the project root"
            ),
        }
    }
}

impl std::error::Error for EvalCommandError {}

impl From<ProjectResolveError> for EvalCommandError {
    fn from(error: ProjectResolveError) -> Self {
        Self::ProjectResolve(error)
    }
}

/// Runs fixture-backed eval comparison for precomputed eval-run contracts.
///
/// # Errors
///
/// Returns an error when project resolution fails, the suite or run fixtures
/// cannot be read or parsed, or a run ref escapes the resolved project root.
pub fn run_compare(
    input: &EvalCompareCommandInput,
) -> Result<EvalComparisonReport, EvalCommandError> {
    let resolved = resolve_project(&input.root, input.allow_bootstrap_core)?;
    let project_root = PathBuf::from(&resolved.project_root);
    let canonical_project_root = fs::canonicalize(&project_root).map_err(|source| {
        EvalCommandError::CanonicalizeProjectRoot {
            path: project_root.clone(),
            source: source.to_string(),
        }
    })?;
    let suite_path = resolve_project_relative_path(
        &project_root,
        &canonical_project_root,
        input
            .suite_path
            .as_deref()
            .unwrap_or_else(|| Path::new(DEFAULT_EVAL_COMPARE_SUITE)),
    )?;
    let suite_document = read_suite(&suite_path)?;
    let suite = suite_document.eval_compare_suite;
    let baseline_runs = read_run_inputs(
        &project_root,
        &canonical_project_root,
        &suite.baseline.run_refs,
    )?;
    let candidate_runs = read_run_inputs(
        &project_root,
        &canonical_project_root,
        &suite.candidate.run_refs,
    )?;
    let evidence_diagnostics = evidence_ref_diagnostics(
        &project_root,
        &canonical_project_root,
        "baseline",
        &baseline_runs,
    )
    .into_iter()
    .chain(evidence_ref_diagnostics(
        &project_root,
        &canonical_project_root,
        "candidate",
        &candidate_runs,
    ))
    .collect();

    Ok(compare_eval_runs_with_diagnostics(
        &suite,
        input.baseline,
        input.candidate,
        &baseline_runs,
        &candidate_runs,
        evidence_diagnostics,
    ))
}

fn read_suite(path: &Path) -> Result<EvalCompareSuiteDocument, EvalCommandError> {
    let text = fs::read_to_string(path).map_err(|source| EvalCommandError::ReadSuite {
        path: path.to_path_buf(),
        source: source.to_string(),
    })?;
    let document: EvalCompareSuiteDocument =
        yaml_serde::from_str(strip_utf8_bom(&text)).map_err(|source| {
            EvalCommandError::ParseSuite {
                path: path.to_path_buf(),
                source: source.to_string(),
            }
        })?;
    if document.schema_version != "0.1" {
        return Err(EvalCommandError::UnsupportedSuiteSchemaVersion {
            path: path.to_path_buf(),
            found: document.schema_version,
        });
    }
    Ok(document)
}

fn read_run_inputs(
    project_root: &Path,
    canonical_project_root: &Path,
    refs: &[RepoPath],
) -> Result<Vec<EvalRunInput>, EvalCommandError> {
    refs.iter()
        .map(|reference| read_run_input(project_root, canonical_project_root, reference))
        .collect()
}

fn read_run_input(
    project_root: &Path,
    canonical_project_root: &Path,
    reference: &RepoPath,
) -> Result<EvalRunInput, EvalCommandError> {
    let path = resolve_safe_repo_file_path(
        project_root,
        canonical_project_root,
        &reference.0,
        EvalRepoFileKind::Run,
    )?;
    let text = fs::read_to_string(&path).map_err(|source| EvalCommandError::ReadRun {
        path: path.clone(),
        source: source.to_string(),
    })?;
    let document: EvalRunContractDocument =
        yaml_serde::from_str(strip_utf8_bom(&text)).map_err(|source| {
            EvalCommandError::ParseRun {
                path: path.clone(),
                source: source.to_string(),
            }
        })?;
    if document.schema_version != "0.1" {
        return Err(EvalCommandError::UnsupportedRunSchemaVersion {
            path,
            found: document.schema_version,
        });
    }
    Ok(EvalRunInput {
        source_ref: reference.clone(),
        document,
    })
}

fn resolve_project_relative_path(
    project_root: &Path,
    canonical_project_root: &Path,
    path: &Path,
) -> Result<PathBuf, EvalCommandError> {
    if path.is_absolute() {
        Err(EvalCommandError::InvalidSuitePath {
            path: path.display().to_string(),
        })
    } else {
        resolve_safe_repo_file_path(
            project_root,
            canonical_project_root,
            &path.to_string_lossy(),
            EvalRepoFileKind::Suite,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalRepoFileKind {
    Suite,
    Run,
}

impl EvalRepoFileKind {
    fn invalid_error(self, path: impl Into<String>) -> EvalCommandError {
        match self {
            Self::Suite => EvalCommandError::InvalidSuitePath { path: path.into() },
            Self::Run => EvalCommandError::InvalidRunPath { path: path.into() },
        }
    }
}

fn resolve_safe_repo_file_path(
    project_root: &Path,
    canonical_project_root: &Path,
    relative_path: &str,
    kind: EvalRepoFileKind,
) -> Result<PathBuf, EvalCommandError> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(forbidden_relative_component)
    {
        return Err(kind.invalid_error(relative_path.to_string()));
    }
    let joined = project_root.join(path);
    if !joined.exists() {
        return Ok(joined);
    }
    if !joined.is_file() {
        return Err(kind.invalid_error(relative_path.to_string()));
    }
    let canonical =
        fs::canonicalize(&joined).map_err(|_| kind.invalid_error(relative_path.to_string()))?;
    if canonical.starts_with(canonical_project_root) {
        Ok(canonical)
    } else {
        Err(kind.invalid_error(relative_path.to_string()))
    }
}

fn forbidden_relative_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Prefix(_) | Component::RootDir | Component::ParentDir
    )
}

fn strip_utf8_bom(raw: &str) -> &str {
    raw.strip_prefix('\u{feff}').unwrap_or(raw)
}

fn evidence_ref_diagnostics(
    project_root: &Path,
    canonical_project_root: &Path,
    arm_path: &str,
    runs: &[EvalRunInput],
) -> Vec<EvalDiagnostic> {
    runs.iter()
        .flat_map(|run| {
            run.document
                .eval_run_contract
                .evidence_refs
                .iter()
                .enumerate()
                .filter_map(move |(index, reference)| {
                    evidence_ref_diagnostic(
                        project_root,
                        canonical_project_root,
                        arm_path,
                        run,
                        index,
                        reference,
                    )
                })
        })
        .collect()
}

fn evidence_ref_diagnostic(
    project_root: &Path,
    canonical_project_root: &Path,
    arm_path: &str,
    run: &EvalRunInput,
    index: usize,
    reference: &str,
) -> Option<EvalDiagnostic> {
    match validate_evidence_ref(project_root, canonical_project_root, reference) {
        Ok(()) => None,
        Err(EvidenceRefValidationError::Invalid) => Some(EvalDiagnostic::error(
            EvalDiagnosticCode::InvalidEvidenceRef,
            evidence_ref_path(arm_path, run, index),
            format!(
                "eval run {} has invalid evidence ref '{}'; refs must be relative file paths under the project root",
                run.document.eval_run_contract.run_id.0, reference
            ),
        )),
        Err(EvidenceRefValidationError::Missing) => Some(EvalDiagnostic::error(
            EvalDiagnosticCode::MissingEvidenceFile,
            evidence_ref_path(arm_path, run, index),
            format!(
                "eval run {} evidence ref '{}' does not exist",
                run.document.eval_run_contract.run_id.0, reference
            ),
        )),
        Err(EvidenceRefValidationError::NotFile) => Some(EvalDiagnostic::error(
            EvalDiagnosticCode::EvidenceRefNotFile,
            evidence_ref_path(arm_path, run, index),
            format!(
                "eval run {} evidence ref '{}' is not a file",
                run.document.eval_run_contract.run_id.0, reference
            ),
        )),
        Err(EvidenceRefValidationError::EscapesProject) => Some(EvalDiagnostic::error(
            EvalDiagnosticCode::EvidenceRefEscapesProject,
            evidence_ref_path(arm_path, run, index),
            format!(
                "eval run {} evidence ref '{}' resolves outside the project root",
                run.document.eval_run_contract.run_id.0, reference
            ),
        )),
    }
}

fn evidence_ref_path(arm_path: &str, run: &EvalRunInput, index: usize) -> String {
    format!(
        "eval_compare_suite.{arm_path}.run_refs.{}.evidence_refs[{index}]",
        run.source_ref.0
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvidenceRefValidationError {
    Invalid,
    Missing,
    NotFile,
    EscapesProject,
}

fn validate_evidence_ref(
    project_root: &Path,
    canonical_project_root: &Path,
    reference: &str,
) -> Result<(), EvidenceRefValidationError> {
    let relative = Path::new(reference);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(forbidden_relative_component)
    {
        return Err(EvidenceRefValidationError::Invalid);
    }

    let joined = project_root.join(relative);
    if !joined.exists() {
        return Err(EvidenceRefValidationError::Missing);
    }
    if !joined.is_file() {
        return Err(EvidenceRefValidationError::NotFile);
    }

    let canonical_evidence =
        fs::canonicalize(&joined).map_err(|_| EvidenceRefValidationError::Missing)?;
    if canonical_evidence.starts_with(canonical_project_root) {
        Ok(())
    } else {
        Err(EvidenceRefValidationError::EscapesProject)
    }
}
/// Dispatch entrypoint for the `forge-core eval` subcommand tree.
///
/// Routes to `compare` based on `args[1]`, and prints usage on `--help` /
/// unknown subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` when the subcommand is unknown or argument
/// parsing fails.
pub fn run_eval_command(args: &[String]) -> Result<(), ExitError> {
    let subcommand = args.get(1).map_or("--help", String::as_str);
    match subcommand {
        "compare" => {
            if args.iter().any(|a| matches!(a.as_str(), "--help" | "-h")) {
                println!("{}", eval_usage());
                return Ok(());
            }
            let (input, json) = parse_eval_compare_args(args)?;
            run_eval_compare(&input, json)
        }
        "--help" | "-h" | "help" => {
            println!("{}", eval_usage());
            Ok(())
        }
        _ => Err(ExitError::usage(eval_usage())),
    }
}

/// Parses argv into a typed [`EvalCompareCommandInput`] plus a JSON flag.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present, or
/// `ExitError::invalid_value` when `--baseline` or `--candidate` is missing,
/// or when any underlying value helper reports a missing or malformed value.
pub fn parse_eval_compare_args(
    args: &[String],
) -> Result<(EvalCompareCommandInput, bool), ExitError> {
    let mut root = PathBuf::from(".");
    let mut suite_path: Option<PathBuf> = None;
    let mut baseline: Option<EvalArmLabel> = None;
    let mut candidate: Option<EvalArmLabel> = None;
    let mut allow_bootstrap_core = false;
    let mut json = false;
    let mut index = 2usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                root = PathBuf::from(next_eval_value_or_err(args, index, "root")?);
            }
            "--suite" => {
                index += 1;
                suite_path = Some(PathBuf::from(next_eval_value_or_err(args, index, "suite")?));
            }
            "--baseline" => {
                index += 1;
                baseline = Some(parse_eval_arm_or_err(
                    next_eval_value_or_err(args, index, "baseline")?,
                    "baseline",
                )?);
            }
            "--candidate" => {
                index += 1;
                candidate = Some(parse_eval_arm_or_err(
                    next_eval_value_or_err(args, index, "candidate")?,
                    "candidate",
                )?);
            }
            "--allow-bootstrap-core" => allow_bootstrap_core = true,
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => break,
            _ => {
                return Err(ExitError::usage(eval_usage()));
            }
        }
        index += 1;
    }
    let baseline = baseline.ok_or_else(|| {
        ExitError::invalid_value("eval compare requires --baseline <single-agent|graph|mas|manual>")
    })?;
    let candidate = candidate.ok_or_else(|| {
        ExitError::invalid_value(
            "eval compare requires --candidate <single-agent|graph|mas|manual>",
        )
    })?;

    Ok((
        EvalCompareCommandInput {
            root,
            suite_path,
            baseline,
            candidate,
            allow_bootstrap_core,
        },
        json,
    ))
}

/// Reads the value at `args[index]`, rejecting missing slots and values
/// that look like the next flag.
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `index` is out of bounds or the
/// value at `index` starts with `-`.
pub fn next_eval_value_or_err<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, ExitError> {
    let value = args.get(index).ok_or_else(|| {
        ExitError::invalid_value(format!("eval compare: missing value for --{flag}"))
    })?;
    if value.starts_with('-') {
        return Err(ExitError::invalid_value(format!(
            "eval compare: missing value for --{flag}"
        )));
    }
    Ok(value.as_str())
}

/// Parses a CLI string into an [`EvalArmLabel`].
///
/// # Errors
///
/// Returns `ExitError::invalid_value` when `value` does not parse as a
/// valid [`EvalArmLabel`].
pub fn parse_eval_arm_or_err(value: &str, flag: &str) -> Result<EvalArmLabel, ExitError> {
    value.parse::<EvalArmLabel>().map_err(|error| {
        ExitError::invalid_value(format!("eval compare: invalid value for --{flag}: {error}"))
    })
}

/// Runs the `forge-core eval compare` subcommand body.
///
/// # Errors
///
/// Returns `ExitError::failed` when the underlying compare returns a
/// `Blocked` status, and `ExitError::env_config` when the underlying compare
/// returns an internal error.
///
/// # Panics
///
/// Panics in JSON mode if the compare output cannot be serialized. The
/// output type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_eval_compare(input: &EvalCompareCommandInput, json: bool) -> Result<(), ExitError> {
    match run_compare(input) {
        Ok(output) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).expect("serialize eval compare output")
                );
            } else {
                println!(
                    "forge_core_eval_compare status={:?} baseline={} candidate={} recommendation={:?} tasks={}",
                    output.status,
                    output.baseline,
                    output.candidate,
                    output.recommendation,
                    output.task_count
                );
                for reason in &output.policy_reasons {
                    println!("reason={reason}");
                }
                for gap in &output.measurement_gaps {
                    println!("measurement_gap={gap}");
                }
            }
            if output.status == EvalCompareStatus::Blocked {
                return Err(ExitError::failed("eval compare status blocked"));
            }
            Ok(())
        }
        Err(error) => Err(ExitError::env_config(format!(
            "eval compare failed: {error}"
        ))),
    }
}
