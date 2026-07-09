//! Generic contract CLI helpers.
//!
//! `forge-core contract validate` loads one of the evolve-phase typed-contract
//! documents and reports whether it deserializes as the requested kind. The
//! driver wires this module into the top-level CLI dispatch.

use forge_core_command_surface::COMMAND_CONTRACT;
use forge_core_contracts::{
    AgentRunContractDocument, AssuranceCaseDocument, AutonomyPolicyContractDocument,
    CheckpointContractDocument, CliEnvelope, EvalRunContractDocument, ExitReason,
    MemoryContractDocument, TelemetryContractDocument, VerificationGoalContractDocument,
};
use forge_core_validate::validate_assurance_case;

use crate::cli_error::ExitError;

const SUPPORTED_KINDS: &[&str] = &[
    "autonomy_policy",
    "verification_goal",
    "agent_run",
    "memory",
    "checkpoint",
    "eval_run",
    "telemetry",
    "assurance_case",
];

/// Successful `contract validate` payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ContractValidationResult {
    pub kind: String,
    pub valid: bool,
    pub schema_version: String,
}

/// Hand-rolled error enum for [`validate_kind`] / `parse_document` (no `thiserror`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractValidateError {
    /// The `kind` string did not match any of the supported contract kinds.
    UnsupportedKind {
        kind: String,
        supported: Vec<String>,
    },
    /// The YAML payload could not be deserialized as the typed document for `kind`.
    YamlInvalid { kind: String, source: String },
    /// The typed document deserialized but violated semantic invariants.
    SemanticInvalid { kind: String, source: String },
}

impl std::fmt::Display for ContractValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedKind { kind, supported } => write!(
                f,
                "unsupported contract kind '{kind}'. Supported kinds: {}",
                supported.join(", ")
            ),
            Self::YamlInvalid { kind, source } => write!(
                f,
                "{kind} file is not valid YAML for that contract type: {source}"
            ),
            Self::SemanticInvalid { kind, source } => {
                write!(f, "{kind} file violates semantic invariants: {source}")
            }
        }
    }
}

/// Parse and run `forge-core contract <subcommand>`.
/// Dispatch entrypoint for the `forge-core contract` subcommand tree.
///
/// Routes to `validate` based on `args[1]`, and prints usage on `--help`
/// / unknown subcommand.
///
/// # Errors
///
/// Returns `ExitError::usage` when the subcommand is unknown or argument
/// parsing fails.
pub fn run_contract_command(args: &[String]) -> Result<(), ExitError> {
    let sub = args.get(1).map_or("--help", String::as_str);
    match sub {
        "validate" => run_validate(&args[2..]),
        "--help" | "-h" | "help" => {
            println!("{}", contract_usage());
            println!("  supported kinds: {}", SUPPORTED_KINDS.join(", "));
            Ok(())
        }
        other => Err(ExitError::usage(format!(
            "forge-core contract: unknown subcommand '{other}'. Try: {hint}",
            hint = contract_subcommand_hint()
        ))),
    }
}

fn contract_usage() -> String {
    let mut usage = String::from("forge-core contract <subcommand> [options]");
    for line in COMMAND_CONTRACT.local_usage_lines() {
        usage.push('\n');
        usage.push_str("  ");
        usage.push_str(line);
    }
    usage
}

fn contract_subcommand_hint() -> String {
    COMMAND_CONTRACT.concrete_subcommand_hint()
}

fn contract_command_surface_usage_line_for(subcommand: &str) -> &'static str {
    COMMAND_CONTRACT
        .usage_line_for_subcommand(subcommand)
        .unwrap_or("forge-core contract <subcommand> [options]")
}

fn contract_validate_usage() -> String {
    format!(
        "usage:\n  {}",
        contract_command_surface_usage_line_for("validate")
    )
}

/// Handler for `forge-core contract validate`.
///
/// # Errors
///
/// Returns `ExitError::usage` when an unknown flag is present or required
/// arguments are missing, and `ExitError::with_code` (via envelope
/// emission) when the underlying validation surfaces a non-zero exit code.
pub fn run_validate(args: &[String]) -> Result<(), ExitError> {
    let mut kind: Option<String> = None;
    let mut file: Option<std::path::PathBuf> = None;
    let mut want_json = true;

    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--kind" => {
                idx += 1;
                kind = Some(require_value(args, idx)?);
            }
            "--file" => {
                idx += 1;
                file = Some(std::path::PathBuf::from(require_value(args, idx)?));
            }
            "--json" => want_json = true,
            "--no-json" | "--text" => want_json = false,
            "--help" | "-h" => {
                println!("{}", contract_command_surface_usage_line_for("validate"));
                println!("supported kinds: {}", SUPPORTED_KINDS.join(", "));
                return Ok(());
            }
            _ => {
                return Err(ExitError::usage(contract_validate_usage()));
            }
        }
        idx += 1;
    }

    let kind = kind.ok_or_else(|| ExitError::usage(contract_validate_usage()))?;
    let file = file.ok_or_else(|| ExitError::usage(contract_validate_usage()))?;

    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) => {
            return Err(emit_err(
                "contract validate",
                &format!("cannot read contract file '{}': {e}", file.display()),
                want_json,
            ));
        }
    };

    match validate_kind(&kind, &text) {
        Ok(payload) => {
            crate::cli_util::emit_envelope(CliEnvelope::ok("contract validate", payload), want_json)
        }
        Err(message) => Err(emit_err(
            "contract validate",
            &message.to_string(),
            want_json,
        )),
    }
}

/// Validate a typed-contract YAML string by kind without exiting the process.
///
/// This deliberately matches on the explicit kind string because the document
/// wrappers are concrete Rust types rather than a shared trait object.
/// Validates a contract document of `kind` against its typed schema.
///
/// # Errors
///
/// Returns [`ContractValidateError::UnsupportedKind`] when `kind` is not
/// in `SUPPORTED_KINDS`, and parse/contract-validation variants when the
/// document is malformed or violates the schema.
pub fn validate_kind(
    kind: &str,
    text: &str,
) -> Result<ContractValidationResult, ContractValidateError> {
    match kind {
        "autonomy_policy" => parse_document::<AutonomyPolicyContractDocument>(kind, text),
        "verification_goal" => parse_document::<VerificationGoalContractDocument>(kind, text),
        "agent_run" => parse_document::<AgentRunContractDocument>(kind, text),
        "memory" => parse_document::<MemoryContractDocument>(kind, text),
        "checkpoint" => parse_document::<CheckpointContractDocument>(kind, text),
        "eval_run" => parse_document::<EvalRunContractDocument>(kind, text),
        "telemetry" => parse_document::<TelemetryContractDocument>(kind, text),
        "assurance_case" => parse_assurance_case(kind, text),
        other => Err(ContractValidateError::UnsupportedKind {
            kind: other.to_string(),
            supported: SUPPORTED_KINDS.iter().map(|s| (*s).to_string()).collect(),
        }),
    }
}

fn parse_assurance_case(
    kind: &str,
    text: &str,
) -> Result<ContractValidationResult, ContractValidateError> {
    let document: AssuranceCaseDocument =
        yaml_serde::from_str(text).map_err(|error| ContractValidateError::YamlInvalid {
            kind: kind.to_owned(),
            source: error.to_string(),
        })?;
    let report = validate_assurance_case(&document);
    if report.has_errors() {
        return Err(ContractValidateError::SemanticInvalid {
            kind: kind.to_owned(),
            source: report
                .diagnostics()
                .iter()
                .map(|diagnostic| {
                    format!(
                        "{:?} at {}: {}",
                        diagnostic.code, diagnostic.path, diagnostic.message
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    Ok(ContractValidationResult {
        kind: kind.to_owned(),
        valid: true,
        schema_version: document.schema_version,
    })
}

fn parse_document<T>(
    kind: &str,
    text: &str,
) -> Result<ContractValidationResult, ContractValidateError>
where
    T: serde::de::DeserializeOwned + HasSchemaVersion,
{
    let doc: T = yaml_serde::from_str(text).map_err(|e| ContractValidateError::YamlInvalid {
        kind: kind.to_string(),
        source: e.to_string(),
    })?;
    Ok(ContractValidationResult {
        kind: kind.to_owned(),
        valid: true,
        schema_version: doc.schema_version().to_owned(),
    })
}

trait HasSchemaVersion {
    fn schema_version(&self) -> &str;
}

impl HasSchemaVersion for AutonomyPolicyContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for VerificationGoalContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for AgentRunContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for MemoryContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for CheckpointContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for EvalRunContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for TelemetryContractDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

impl HasSchemaVersion for AssuranceCaseDocument {
    fn schema_version(&self) -> &str {
        &self.schema_version
    }
}

fn require_value(args: &[String], idx: usize) -> Result<String, ExitError> {
    args.get(idx)
        .cloned()
        .ok_or_else(|| ExitError::usage(contract_validate_usage()))
}

fn emit_err(command: &str, message: &str, want_json: bool) -> ExitError {
    let env: CliEnvelope<()> = CliEnvelope::err(command, ExitReason::InvalidDecisionShape, message);
    // Delegate to the single emit path; unwrap the Ok (unreachable here — the
    // envelope is always an error, so emit_envelope returns WithCode) and
    // surface the typed ExitError. The caller wraps this in `Err(...)`.
    match crate::cli_util::emit_envelope(env, want_json) {
        Ok(()) => ExitError::with_code(
            ExitReason::InvalidDecisionShape.as_code(),
            "emit_envelope Ok path is unreachable: envelope is always an error here".to_string(),
        ),
        Err(err) => err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_autonomy_policy_yaml() -> &'static str {
        r#"
schema_version: "0.1"
autonomy_policy_contract:
  id: autonomy.fast_lane.default
  applies_to:
    kind: lane
    ids:
      - fast_lane
  default_mode: sandbox_auto
  tool_classes: []
  escalation:
    on_repeated_failure: 2
    on_high_risk_path: true
    on_semantic_uncertainty: true
    max_retries_before_human: 3
    cooldown_seconds: 300
  evidence_basis: null
"#
    }

    #[test]
    fn valid_autonomy_policy_yaml_is_accepted() {
        let path = std::env::temp_dir().join(format!(
            "forge-contract-cmd-autonomy-policy-{}.yaml",
            std::process::id()
        ));
        std::fs::write(&path, valid_autonomy_policy_yaml()).expect("write temp autonomy policy");
        let text = std::fs::read_to_string(&path).expect("read temp autonomy policy");
        let _ = std::fs::remove_file(&path);

        let result = validate_kind("autonomy_policy", &text).expect("valid autonomy policy");

        assert_eq!(result.kind, "autonomy_policy");
        assert!(result.valid);
        assert_eq!(result.schema_version, "0.1");
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let err = validate_kind("not_a_contract", valid_autonomy_policy_yaml())
            .expect_err("unknown kind must fail");

        assert!(
            err.to_string().contains("unsupported contract kind"),
            "got: {err}"
        );
    }

    #[test]
    fn malformed_yaml_is_rejected() {
        let err = validate_kind("autonomy_policy", "schema_version: [")
            .expect_err("malformed yaml must fail");

        assert!(
            err.to_string()
                .contains("autonomy_policy file is not valid YAML"),
            "got: {err}"
        );
    }

    #[test]
    fn valid_assurance_case_is_accepted_with_semantic_validation() {
        let yaml = include_str!(
            "../../../contracts/assurance/representative-slice-verified-assurance.yaml"
        );

        let result = validate_kind("assurance_case", yaml).expect("valid Assurance Case");

        assert_eq!(result.kind, "assurance_case");
        assert!(result.valid);
        assert_eq!(result.schema_version, "0.1");
    }

    #[test]
    fn semantically_invalid_assurance_case_is_rejected() {
        let yaml = include_str!(
            "../../../contracts/assurance/representative-slice-verified-assurance.yaml"
        )
        .replacen("schema_version: \"0.1\"", "schema_version: \"999\"", 1);

        let error = validate_kind("assurance_case", &yaml)
            .expect_err("unsupported Assurance Case schema must fail");

        assert!(
            error.to_string().contains("semantic invariants"),
            "got: {error}"
        );
    }

    #[test]
    fn contract_usage_projects_command_surface_lines() {
        let usage = contract_usage();
        assert!(
            usage.starts_with("forge-core contract <subcommand> [options]"),
            "contract usage should keep the local command-tree header: {usage}"
        );
        for line in COMMAND_CONTRACT.usage_lines {
            let subcommand_usage = COMMAND_CONTRACT.local_usage_line(line);
            assert!(
                usage.contains(subcommand_usage),
                "contract usage should include projected Command Surface line {subcommand_usage:?}: {usage}"
            );
        }
        assert_eq!(contract_subcommand_hint(), "validate");
    }

    #[test]
    fn contract_subcommand_help_lookup_projects_full_command_surface_lines() {
        assert_eq!(
            contract_command_surface_usage_line_for("validate"),
            COMMAND_CONTRACT.canonical_usage().trim_start()
        );
    }

    #[test]
    fn missing_contract_validate_flag_value_reports_command_surface_usage() {
        let error =
            run_validate(&args(&["--kind"])).expect_err("missing kind value must fail before I/O");
        assert_contract_validate_usage_error(&error);
    }

    #[test]
    fn missing_contract_validate_required_flags_report_command_surface_usage() {
        let error =
            run_validate(&args(&[])).expect_err("missing kind and file must fail before I/O");
        assert_contract_validate_usage_error(&error);
    }

    #[test]
    fn unknown_contract_validate_arg_reports_command_surface_usage() {
        let error = run_validate(&args(&["--frobnicate"]))
            .expect_err("unknown argument must fail before I/O");
        assert_contract_validate_usage_error(&error);
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn assert_contract_validate_usage_error(error: &ExitError) {
        let projected = COMMAND_CONTRACT.canonical_usage().trim_start();
        assert!(
            error.message().contains(projected),
            "contract validate usage error should include projected Command Surface line {projected:?}: {error}"
        );
        assert!(
            !error.message().contains("forge-core execute-operation"),
            "contract validate usage error must not include unrelated mutating command usage: {error}"
        );
    }
}
