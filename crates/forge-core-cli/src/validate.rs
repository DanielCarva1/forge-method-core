//! Forge workspace contract validator.
//!
//! Walks the repository under a given root, parses every YAML contract
//! (commands, operations, side contracts, runtime contracts, evidence
//! registry, contract family inventory) and accumulates diagnostics into a
//! [`ValidateSummary`]. The summary is the regression anchor used by
//! `forge-core-cli validate --json` and by the parity tests in
//! `forge-contract-validator`. The shape of the JSON output MUST stay
//! stable; refactors here are behavior-preserving.

use crate::cli_error::ExitError;
use crate::cli_util::command_surface_usage;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use tracing::instrument;

use forge_core_command_surface::{CommandSpec, COMMAND_VALIDATE};
use forge_core_contracts::{
    ClaimContractDocument, CommandContractDocument, CompletionContractDocument,
    ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument, RequestContractDocument,
    RuntimeCapabilityDocument, RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument,
    ToolEffectContractDocument,
};
use forge_core_store::{collect_known_repo_paths, collect_validation_yaml_documents};
use forge_core_validate::{
    validate_claim, validate_claim_cross_references, validate_command, validate_completion,
    validate_completion_cross_references, validate_coordination_eval,
    validate_coordination_eval_cross_references, validate_decision_close,
    validate_decision_close_cross_references, validate_evidence_registry, validate_gate,
    validate_gate_cross_references, validate_health_recovery,
    validate_health_recovery_cross_references, validate_inventory, validate_inventory_references,
    validate_operation, validate_operation_cross_references, validate_request,
    validate_request_cross_references, validate_runtime_capability, validate_runtime_handoff,
    validate_runtime_handoff_cross_references, validate_runtime_registry_cross_references,
    validate_runtime_registry_entry, validate_tool_effect, validate_tool_effect_cross_references,
    validate_yaml_known_repo_references, validate_yaml_source_id_references, Diagnostic,
    DiagnosticCode, DiagnosticSeverity, ReferenceIndex, ValidationReport,
};

/// Outcome of a single named validation check (passed/failed + counts).
#[derive(Debug, Clone, Serialize)]
pub struct ValidateCheck {
    pub name: String,
    pub status: ValidationStatus,
    pub diagnostics: usize,
    pub errors: usize,
}

/// One diagnostic emitted while validating the workspace.
///
/// Migrated in V2.B: this is now an alias for the canonical
/// `forge_core_validate::Diagnostic`, which keeps `DiagnosticSeverity` and
/// `DiagnosticCode` as the **strong enums** end-to-end instead of degrading
/// them to `String` at this boundary. The serialized JSON shape
/// (`{ severity, code, path, message }`) is unchanged — `severity` is still
/// `"error"`/`"warning"` and `code` is still a stable `snake_case` identifier,
/// but now produced by `DiagnosticCode`'s serde rename rather than the lossy
/// `format!("{:?}", diagnostic.code)` that previously emitted `PascalCase`
/// `Debug` strings (e.g. `YamlReadFailed`) and discarded the enum typing.
pub type ValidateDiagnostic = forge_core_validate::Diagnostic;

/// Aggregated result of validating a Forge workspace. Fields are public so
/// callers (`forge-contract-validator`, integration tests, `main.rs`) can
/// read status/checks/diagnostics directly. JSON output is produced by
/// `Serialize` and must remain stable.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateSummary {
    pub status: ValidationStatus,
    pub root: String,
    pub checks: Vec<ValidateCheck>,
    pub diagnostics: Vec<ValidateDiagnostic>,
}

/// Top-level pass/fail status for a workspace validation run.
///
/// Serializes as lowercase (`"passed"` / `"failed"`) to match the rest of
/// the workspace's JSON contract (`gate_status`, `coordination` verdicts,
/// trace `gate` events). The original `PascalCase` emit was inconsistent
/// with every other status field in the binary.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    Passed,
    Failed,
}

impl ValidateSummary {
    /// Returns `true` when every check passed and no error diagnostics were
    /// collected.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.status == ValidationStatus::Passed
    }

    /// One-line human-readable summary used by the legacy validator bridge.
    #[must_use]
    pub fn human_summary(&self) -> String {
        if self.passed() {
            format!(
                "forge_core_validation_passed checks={} diagnostics=0",
                self.checks.len()
            )
        } else {
            format!(
                "forge_core_validation_failed checks={} diagnostics={}",
                self.checks.len(),
                self.diagnostics.len()
            )
        }
    }

    fn add_report(&mut self, name: &str, report: ValidationReport) {
        let errors = report.error_count();
        let diagnostics = report.diagnostics().len();
        // V2.B: store the typed `Diagnostic` directly. Previously this demoted
        // the strong `DiagnosticCode` enum to a `format!("{:?}")` `Debug` string
        // via `ValidateDiagnostic::from_validation`, losing type information at
        // the boundary. Now the canonical enum flows straight into the summary.
        self.diagnostics
            .extend(report.diagnostics().iter().cloned());
        self.checks.push(ValidateCheck {
            name: name.to_string(),
            status: if errors == 0 {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            diagnostics,
            errors,
        });
    }

    fn add_validation_diagnostics(&mut self, name: &str, diagnostics: &[Diagnostic]) {
        let errors = diagnostics
            .iter()
            .filter(|item| item.severity == DiagnosticSeverity::Error)
            .count();
        // V2.B: clone the typed diagnostics in directly (no lossy conversion).
        self.diagnostics.extend(diagnostics.iter().cloned());
        self.checks.push(ValidateCheck {
            name: name.to_string(),
            status: if errors == 0 {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            diagnostics: diagnostics.len(),
            errors,
        });
    }

    fn push_diagnostic(&mut self, diagnostic: ValidateDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn finish(&mut self) {
        // V2.B: diagnostics are now typed `Diagnostic`, so compare against the
        // strong `DiagnosticSeverity::Error` enum rather than a `"error"` string.
        self.status = if self
            .diagnostics
            .iter()
            .any(|item| item.severity == DiagnosticSeverity::Error)
        {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        };
    }
}

/// Detect whether `root` is a consumer project (no local `contracts/` tree)
/// versus the Forge core repo (which owns the canonical contracts).
///
/// A consumer created by `forge-core project init` ships no `contracts/`
/// directory: it carries only the `.forge-method.yaml` pointer and its runtime
/// state lives in the sibling sidecar. The shared contract definitions are
/// served from the binary (embedded), and the core-only fixtures
/// (`contracts/runtimes`, `docs/fixtures/operation-contract-v0`, the evidence
/// registry, the inventory) are not present. This predicate gates those
/// core-only validation passes so a consumer gets a clean validation rather
/// than hard errors for files it is not supposed to ship.
fn is_consumer_repo(root: &Path) -> bool {
    !root.join("contracts").is_dir()
}

/// Validate the Forge workspace rooted at `root`. Walks the contract tree,
/// builds a reference index, parses every YAML document, and accumulates
/// diagnostics into a [`ValidateSummary`] whose JSON shape is the regression
/// anchor for `forge-core-cli validate --json`.
pub fn run_validate(root: impl AsRef<Path>) -> ValidateSummary {
    let root = root.as_ref();
    let mut summary = ValidateSummary {
        status: ValidationStatus::Passed,
        root: root.to_string_lossy().into_owned(),
        checks: Vec::new(),
        diagnostics: Vec::new(),
    };

    // Build the reference index. Seed it with the embedded contract paths so
    // a consumer repo that ships no `contracts/` tree still resolves the
    // shared definitions served from the binary (disk copies still win when
    // present, via insert_existing).
    let embedded_refs = forge_core_decisions::embedded_yaml_paths();
    let index = match forge_core_store::ReferenceIndexBuilder::new()
        .with_known_embedded_refs(embedded_refs)
        .build(root)
    {
        Ok(index) => index,
        Err(err) => {
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReferenceIndexBuildFailed,
                "reference_index",
                err.to_string(),
            ));
            summary.finish();
            return summary;
        }
    };
    let yaml_documents = collect_validation_yaml_documents(root);
    let known_repo_paths = collect_known_repo_paths(root);
    summary.add_validation_diagnostics("yaml_parse", &yaml_documents.diagnostics);

    let evidence_path = root.join("contracts/research/field-evidence-20260625.yaml");
    // The evidence registry and inventory are core-only fixtures: a consumer
    // repo never ships them. When the consumer has no local contracts/ tree,
    // read them silently as None (skip the dependent checks) rather than
    // emitting hard errors for files the consumer is not supposed to have.
    let is_consumer = is_consumer_repo(root);
    let evidence = if is_consumer {
        None
    } else {
        read_yaml::<FieldEvidenceRegistry>(&evidence_path, &mut summary)
    };
    if let Some(evidence) = &evidence {
        summary.add_report("evidence_registry", validate_evidence_registry(evidence));
        summary.add_report(
            "yaml_source_id_refs",
            validate_yaml_source_id_references(&yaml_documents.documents, evidence),
        );
    }
    summary.add_report(
        "yaml_known_repo_refs",
        validate_yaml_known_repo_references(&yaml_documents.documents, &known_repo_paths),
    );

    let inventory_path = root.join("contracts/inventory/v0-contract-family-lock.yaml");
    let inventory = if is_consumer {
        None
    } else {
        read_yaml::<ContractFamilyInventoryDocument>(&inventory_path, &mut summary)
    };
    if let Some(inventory) = &inventory {
        if let Some(evidence) = &evidence {
            summary.add_report("inventory", validate_inventory(inventory, evidence));
        }
        summary.add_report(
            "inventory_references",
            validate_inventory_references(inventory, &index),
        );
    }

    validate_named_dir_instances::<CommandContractDocument, _>(
        root,
        "contracts/commands",
        "command-contract-v0.yaml",
        "command_contracts",
        &mut summary,
        validate_command,
    );
    // Operation fixtures (docs/fixtures/operation-contract-v0), side-contract
    // instances, and runtime contracts (contracts/runtimes/*.yaml) are
    // core-only fixtures. A consumer repo never ships them, so skip these
    // checks entirely when running against a consumer (no local contracts/
    // tree). The consumer's own instances, if any, live under contracts/ and
    // are already covered by the named-dir-instance + known-repo-ref checks
    // above when present.
    if !is_consumer {
        validate_operation_fixtures(root, &index, &mut summary);
        validate_side_contracts(root, &index, &mut summary);
        validate_runtime_contracts(root, &index, &mut summary);
    }

    summary.finish();
    summary
}

fn validate_operation_fixtures(root: &Path, index: &ReferenceIndex, summary: &mut ValidateSummary) {
    let dir = root.join("docs/fixtures/operation-contract-v0");
    for path in yaml_files(&dir, summary) {
        if let Some(operation) = read_yaml::<OperationContractDocument>(&path, summary) {
            summary.add_report(
                &format!("operation_contract:{}", repo_relative(root, &path)),
                validate_operation(&operation),
            );
            summary.add_report(
                &format!("operation_refs:{}", repo_relative(root, &path)),
                validate_operation_cross_references(&operation, index),
            );
        }
    }
}

fn validate_side_contracts(root: &Path, index: &ReferenceIndex, summary: &mut ValidateSummary) {
    validate_named_dir_instances::<ClaimContractDocument, _>(
        root,
        "contracts/claims",
        "claim-contract-v0.yaml",
        "claim_contract",
        summary,
        validate_claim,
    );
    validate_cross_ref_instances::<ClaimContractDocument, _>(
        root,
        "contracts/claims",
        "claim-contract-v0.yaml",
        "claim_refs",
        summary,
        index,
        validate_claim_cross_references,
    );
    validate_named_dir_instances::<CompletionContractDocument, _>(
        root,
        "contracts/completion",
        "completion-contract-v0.yaml",
        "completion_contract",
        summary,
        validate_completion,
    );
    validate_cross_ref_instances::<CompletionContractDocument, _>(
        root,
        "contracts/completion",
        "completion-contract-v0.yaml",
        "completion_refs",
        summary,
        index,
        validate_completion_cross_references,
    );
    validate_named_dir_instances::<GateContractDocument, _>(
        root,
        "contracts/gates",
        "gate-contract-v0.yaml",
        "gate_contract",
        summary,
        validate_gate,
    );
    validate_cross_ref_instances::<GateContractDocument, _>(
        root,
        "contracts/gates",
        "gate-contract-v0.yaml",
        "gate_refs",
        summary,
        index,
        validate_gate_cross_references,
    );
    validate_named_dir_instances::<RequestContractDocument, _>(
        root,
        "contracts/requests",
        "request-contract-v0.yaml",
        "request_contract",
        summary,
        validate_request,
    );
    validate_cross_ref_instances::<RequestContractDocument, _>(
        root,
        "contracts/requests",
        "request-contract-v0.yaml",
        "request_refs",
        summary,
        index,
        validate_request_cross_references,
    );
    validate_named_dir_instances::<ToolEffectContractDocument, _>(
        root,
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        "tool_effect_contract",
        summary,
        validate_tool_effect,
    );
    validate_cross_ref_instances::<ToolEffectContractDocument, _>(
        root,
        "contracts/effects",
        "tool-effect-contract-v0.yaml",
        "tool_effect_refs",
        summary,
        index,
        validate_tool_effect_cross_references,
    );
    validate_named_dir_instances::<DecisionCloseContractDocument, _>(
        root,
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        "decision_close_contract",
        summary,
        validate_decision_close,
    );
    validate_cross_ref_instances::<DecisionCloseContractDocument, _>(
        root,
        "contracts/decisions",
        "decision-close-contract-v0.yaml",
        "decision_close_refs",
        summary,
        index,
        validate_decision_close_cross_references,
    );
    validate_named_dir_instances::<HealthRecoveryContractDocument, _>(
        root,
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        "health_recovery_contract",
        summary,
        validate_health_recovery,
    );
    validate_cross_ref_instances::<HealthRecoveryContractDocument, _>(
        root,
        "contracts/recovery",
        "health-recovery-contract-v0.yaml",
        "health_recovery_refs",
        summary,
        index,
        validate_health_recovery_cross_references,
    );
    validate_named_dir_instances::<CoordinationEvalContractDocument, _>(
        root,
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        "coordination_eval_contract",
        summary,
        validate_coordination_eval,
    );
    validate_cross_ref_instances::<CoordinationEvalContractDocument, _>(
        root,
        "contracts/evals",
        "coordination-eval-contract-v0.yaml",
        "coordination_eval_refs",
        summary,
        index,
        validate_coordination_eval_cross_references,
    );
}

fn validate_runtime_contracts(root: &Path, index: &ReferenceIndex, summary: &mut ValidateSummary) {
    validate_named::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        "runtime_handoff_contract",
        summary,
        validate_runtime_handoff,
    );
    validate_named::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        "runtime_handoff_contract",
        summary,
        validate_runtime_handoff,
    );
    validate_named_cross::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-runtime.yaml",
        "runtime_handoff_refs",
        summary,
        index,
        validate_runtime_handoff_cross_references,
    );
    validate_named_cross::<RuntimeHandoffContractDocument, _>(
        root,
        "contracts/runtimes/cursor-browser-validation-missing-capability.yaml",
        "runtime_handoff_refs",
        summary,
        index,
        validate_runtime_handoff_cross_references,
    );
    validate_named::<RuntimeRegistryEntryDocument, _>(
        root,
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        "runtime_registry_entry",
        summary,
        validate_runtime_registry_entry,
    );
    validate_named_cross::<RuntimeRegistryEntryDocument, _>(
        root,
        "contracts/runtimes/registry-cursor-browser-agent.yaml",
        "runtime_registry_refs",
        summary,
        index,
        validate_runtime_registry_cross_references,
    );
    validate_named::<RuntimeCapabilityDocument, _>(
        root,
        "contracts/runtimes/capability-browser-validation.yaml",
        "runtime_capability",
        summary,
        validate_runtime_capability,
    );
}

fn validate_named_dir_instances<T, F>(
    root: &Path,
    relative_dir: &str,
    definition_file: &str,
    check_prefix: &str,
    summary: &mut ValidateSummary,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> ValidationReport,
{
    let dir = root.join(relative_dir);
    for path in yaml_files(&dir, summary) {
        if path.file_name().and_then(|value| value.to_str()) == Some(definition_file) {
            continue;
        }
        if let Some(contract) = read_yaml::<T>(&path, summary) {
            summary.add_report(
                &format!("{check_prefix}:{}", repo_relative(root, &path)),
                validate(&contract),
            );
        }
    }
}

fn validate_cross_ref_instances<T, F>(
    root: &Path,
    relative_dir: &str,
    definition_file: &str,
    check_prefix: &str,
    summary: &mut ValidateSummary,
    index: &ReferenceIndex,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> ValidationReport,
{
    let dir = root.join(relative_dir);
    for path in yaml_files(&dir, summary) {
        if path.file_name().and_then(|value| value.to_str()) == Some(definition_file) {
            continue;
        }
        if let Some(contract) = read_yaml::<T>(&path, summary) {
            summary.add_report(
                &format!("{check_prefix}:{}", repo_relative(root, &path)),
                validate(&contract, index),
            );
        }
    }
}

fn validate_named<T, F>(
    root: &Path,
    relative_path: &str,
    check_name: &str,
    summary: &mut ValidateSummary,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T) -> ValidationReport,
{
    let path = root.join(relative_path);
    if let Some(contract) = read_yaml::<T>(&path, summary) {
        summary.add_report(
            &format!("{check_name}:{}", repo_relative(root, &path)),
            validate(&contract),
        );
    }
}

fn validate_named_cross<T, F>(
    root: &Path,
    relative_path: &str,
    check_name: &str,
    summary: &mut ValidateSummary,
    index: &ReferenceIndex,
    validate: F,
) where
    T: serde::de::DeserializeOwned,
    F: Fn(&T, &ReferenceIndex) -> ValidationReport,
{
    let path = root.join(relative_path);
    if let Some(contract) = read_yaml::<T>(&path, summary) {
        summary.add_report(
            &format!("{check_name}:{}", repo_relative(root, &path)),
            validate(&contract, index),
        );
    }
}

fn read_yaml<T: serde::de::DeserializeOwned>(
    path: &Path,
    summary: &mut ValidateSummary,
) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReadFileFailed,
                path.to_string_lossy(),
                err.to_string(),
            ));
            return None;
        }
    };
    match yaml_serde::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ParseYamlFailed,
                path.to_string_lossy(),
                err.to_string(),
            ));
            None
        }
    }
}

fn yaml_files(dir: &Path, summary: &mut ValidateSummary) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            // A missing directory is not an error: it means there are no
            // instances to validate (common for a consumer repo that ships
            // no contracts/commands, contracts/claims, etc.). Any other IO
            // error (permission denied, …) is a real failure and surfaces.
            if err.kind() == std::io::ErrorKind::NotFound {
                return Vec::new();
            }
            summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReadDirFailed,
                dir.to_string_lossy(),
                err.to_string(),
            ));
            return Vec::new();
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("yaml") {
                    files.push(path);
                }
            }
            Err(err) => summary.push_diagnostic(Diagnostic::error(
                DiagnosticCode::ReadDirEntryFailed,
                dir.to_string_lossy(),
                err.to_string(),
            )),
        }
    }
    files.sort();
    files
}

fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
#[must_use]
fn validate_usage(command: &CommandSpec) -> String {
    command_surface_usage(command)
}

#[instrument(skip_all, fields(root = tracing::field::Empty, json = tracing::field::Empty, diagnostic_count = tracing::field::Empty), level = "info")]
/// Dispatch entrypoint for the `forge-core validate` command.
///
/// Loads the project at `--root` (default `.`) and prints the resulting
/// [`ValidateSummary`] as JSON (`--json`) or human-readable text.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or `--root`
/// is missing a value.
///
/// # Panics
///
/// Panics in JSON mode if the validation summary cannot be serialized. The
/// summary type derives `Serialize`, so this is a programming error and
/// never occurs on valid input.
pub fn run_validate_command(args: &[String]) -> Result<(), ExitError> {
    let command = &COMMAND_VALIDATE;
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(ExitError::usage(validate_usage(command)));
                };
                root = PathBuf::from(value);
            }
            "--json" => json = true,
            "--no-json" => json = false,
            "--help" | "-h" => {
                println!("{}", validate_usage(command));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(validate_usage(command)));
            }
        }
        index += 1;
    }

    let span = tracing::Span::current();
    span.record("root", root.to_string_lossy().to_string().as_str());
    span.record("json", json);
    let summary = run_validate(&root);
    span.record("diagnostic_count", summary.diagnostics.len());
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary).expect("serialize validation summary")
        );
    } else {
        println!("{}", summary.human_summary());
        for diagnostic in &summary.diagnostics {
            // V2.B: diagnostics are now the typed `Diagnostic`. Print their
            // stable wire-format fields (snake_case `severity`/`code`) by
            // reading them back from the JSON serialization, so the
            // human-readable output matches the `--json` code identifiers
            // rather than the lossy `Debug` form.
            let wire = serde_json::to_value(diagnostic).expect("serialize diagnostic");
            let severity = wire
                .get("severity")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("error");
            let code = wire
                .get("code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            eprintln!(
                "{severity} {code} {}: {}",
                diagnostic.path, diagnostic.message
            );
        }
    }

    if !summary.passed() {
        return Err(ExitError::failed("validation reported errors"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn validate_usage_projects_command_surface_line() {
        let usage = validate_usage(&COMMAND_VALIDATE);
        assert!(usage.starts_with("usage:\n"));
        for line in COMMAND_VALIDATE.usage_lines {
            let projected = format!("  {}", line.trim_start());
            assert!(
                usage.contains(&projected),
                "validate usage should include projected Command Surface line {projected:?}: {usage}"
            );
        }
    }

    #[test]
    fn explicit_no_json_is_accepted_by_validate_help_path() {
        run_validate_command(&args(&["validate", "--no-json", "--help"]))
            .expect("validate accepts explicit --no-json");
    }

    #[test]
    fn missing_root_reports_validate_usage() {
        let error = run_validate_command(&args(&["validate", "--root"]))
            .expect_err("missing root value must fail before validation");
        assert!(
            error.message().contains("forge-core validate"),
            "missing root should report validate usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core execute-operation"),
            "validate usage must not include unrelated global commands: {error}"
        );
    }

    #[test]
    fn unknown_arg_reports_validate_usage() {
        let error = run_validate_command(&args(&["validate", "--frobnicate"]))
            .expect_err("unknown validate argument must fail before validation");
        assert!(
            error.message().contains("forge-core validate"),
            "unknown argument should report validate usage: {error}"
        );
        assert!(
            !error.message().contains("forge-core start"),
            "validate usage must not include unrelated global commands: {error}"
        );
    }
}
