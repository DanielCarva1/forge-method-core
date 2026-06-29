//! Forge workspace contract validator.
//!
//! Walks the repository under a given root, parses every YAML contract
//! (commands, operations, side contracts, runtime contracts, evidence
//! registry, contract family inventory) and accumulates diagnostics into a
//! [`ValidateSummary`]. The summary is the regression anchor used by
//! `forge-core-cli validate --json` and by the parity tests in
//! `forge-contract-validator`. The shape of the JSON output MUST stay
//! stable; refactors here are behavior-preserving.

use crate::cli_util::usage;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use forge_core_contracts::{
    ClaimContractDocument, CommandContractDocument, CompletionContractDocument,
    ContractFamilyInventoryDocument, CoordinationEvalContractDocument,
    DecisionCloseContractDocument, FieldEvidenceRegistry, GateContractDocument,
    HealthRecoveryContractDocument, OperationContractDocument, RequestContractDocument,
    RuntimeCapabilityDocument, RuntimeHandoffContractDocument, RuntimeRegistryEntryDocument,
    ToolEffectContractDocument,
};
use forge_core_store::{
    build_reference_index, collect_known_repo_paths, collect_validation_yaml_documents,
};
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
    DiagnosticSeverity, ReferenceIndex, ValidationReport,
};

/// Outcome of a single named validation check (passed/failed + counts).
#[derive(Debug, Clone, Serialize)]
pub struct ValidateCheck {
    pub name: String,
    pub status: ValidationStatus,
    pub diagnostics: usize,
    pub errors: usize,
}

/// One diagnostic emitted while validating the workspace. The `severity`
/// string is `"error"` or `"warning"` (matches the JSON output shape that
/// external consumers depend on).
#[derive(Debug, Clone, Serialize)]
pub struct ValidateDiagnostic {
    pub severity: String,
    pub code: String,
    pub path: String,
    pub message: String,
}

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
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ValidationStatus {
    Passed,
    Failed,
}

impl ValidateSummary {
    /// Returns `true` when every check passed and no error diagnostics were
    /// collected.
    pub fn passed(&self) -> bool {
        self.status == ValidationStatus::Passed
    }

    /// One-line human-readable summary used by the legacy validator bridge.
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
        let errors = report
            .diagnostics()
            .iter()
            .filter(|item| item.severity == DiagnosticSeverity::Error)
            .count();
        let diagnostics = report.diagnostics().len();
        self.diagnostics.extend(
            report
                .diagnostics()
                .iter()
                .map(ValidateDiagnostic::from_validation),
        );
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
        self.diagnostics
            .extend(diagnostics.iter().map(ValidateDiagnostic::from_validation));
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
        self.status = if self.diagnostics.iter().any(|item| item.severity == "error") {
            ValidationStatus::Failed
        } else {
            ValidationStatus::Passed
        };
    }
}

impl ValidateDiagnostic {
    fn error(code: impl Into<String>, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }

    fn from_validation(diagnostic: &forge_core_validate::Diagnostic) -> Self {
        Self {
            severity: match diagnostic.severity {
                DiagnosticSeverity::Error => "error",
                DiagnosticSeverity::Warning => "warning",
            }
            .to_string(),
            code: format!("{:?}", diagnostic.code),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        }
    }
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

    let index = match build_reference_index(root) {
        Ok(index) => index,
        Err(err) => {
            summary.push_diagnostic(ValidateDiagnostic::error(
                "reference_index_build_failed",
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
    let evidence = read_yaml::<FieldEvidenceRegistry>(&evidence_path, &mut summary);
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
    let inventory = read_yaml::<ContractFamilyInventoryDocument>(&inventory_path, &mut summary);
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
    validate_operation_fixtures(root, &index, &mut summary);
    validate_side_contracts(root, &index, &mut summary);
    validate_runtime_contracts(root, &index, &mut summary);

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
            summary.push_diagnostic(ValidateDiagnostic::error(
                "read_file_failed",
                path.to_string_lossy(),
                err.to_string(),
            ));
            return None;
        }
    };
    match serde_yaml::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            summary.push_diagnostic(ValidateDiagnostic::error(
                "parse_yaml_failed",
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
            summary.push_diagnostic(ValidateDiagnostic::error(
                "read_dir_failed",
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
            Err(err) => summary.push_diagnostic(ValidateDiagnostic::error(
                "read_dir_entry_failed",
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
pub fn run_validate_command(args: &[String]) {
    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("{}", usage());
                    std::process::exit(2);
                };
                root = PathBuf::from(value);
            }
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return;
            }
            _ => {
                eprintln!("{}", usage());
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let summary = run_validate(&root);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&summary).expect("serialize validation summary")
        );
    } else {
        println!("{}", summary.human_summary());
        for diagnostic in &summary.diagnostics {
            eprintln!(
                "{} {} {}: {}",
                diagnostic.severity, diagnostic.code, diagnostic.path, diagnostic.message
            );
        }
    }

    if !summary.passed() {
        std::process::exit(1);
    }
}
